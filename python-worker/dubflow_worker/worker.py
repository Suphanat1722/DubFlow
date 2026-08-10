"""Entry point for the DubFlow worker process."""

from __future__ import annotations

import json
import sys
from typing import Any

from .tts import (
    ReferenceArtifact,
    ReferenceInput,
    SynthesisRequest,
    SynthesisSettings,
    TtsError,
)
from .ipc import RpcResponse, encode_response, parse_request


class Worker:
    """Owns the TTS provider lifetime and routes RPC methods."""

    def __init__(self) -> None:
        self._provider: Any | None = None
        self._output_dir: str | None = None

    def handle(self, method: str, params: dict) -> dict:
        if method == "system.ping":
            return {"pong": True}
        if method == "worker.configure":
            output_dir = params.get("outputDir")
            if output_dir is not None:
                import os

                os.makedirs(output_dir, exist_ok=True)
                self._output_dir = output_dir
            return {"outputDir": self._output_dir}
        if method == "tts.initialize":
            if self._provider is not None:
                return {"alreadyInitialized": True}
            from .tts.jaitts_f5tts import JaittsF5TtsProvider

            provider = JaittsF5TtsProvider()
            provider.initialize(params.get("options") or {})
            self._provider = provider
            return {
                "provider": provider.id,
                "version": provider.version,
                "device": provider.device,
            }
        if method == "tts.preprocess_reference":
            if self._provider is None:
                raise ValueError("tts.initialize must be called first")
            ref = ReferenceInput(
                audio_path=params["audioPath"],
                transcript=params.get("transcript") or "",
            )
            artifact = self._provider.preprocess_reference(ref)
            return {
                "audioPath": artifact.audio_path,
                "transcript": artifact.transcript,
                "durationMs": artifact.duration_ms,
                "sampleRate": artifact.sample_rate,
                "sha256": artifact.sha256,
            }
        if method == "tts.synthesize":
            if self._output_dir is None:
                raise ValueError("worker.configure with outputDir must be called before synthesis")
            if self._provider is None:
                raise ValueError("tts.initialize must be called first")
            request = SynthesisRequest(
                reference=ReferenceArtifact(
                    audio_path=params["reference"]["audioPath"],
                    transcript=params["reference"]["transcript"],
                    duration_ms=params["reference"]["durationMs"],
                    sample_rate=params["reference"]["sampleRate"],
                    sha256=params["reference"]["sha256"],
                ),
                text=params["text"],
                seed=int(params["seed"]),
                settings=SynthesisSettings(
                    nfe_step=int(params.get("settings", {}).get("nfeStep", 32)),
                    cfg_strength=float(params.get("settings", {}).get("cfgStrength", 2.0)),
                    sway_sampling_coef=float(
                        params.get("settings", {}).get("swaySamplingCoef", -1.0)
                    ),
                    speed=float(params.get("settings", {}).get("speed", 1.0)),
                    target_rms=float(params.get("settings", {}).get("targetRms", 0.1)),
                ),
                output_dir=self._output_dir,
            )
            result = self._provider.synthesize(request)
            return {
                "audioPath": result.audio_path,
                "durationMs": result.duration_ms,
                "seed": result.seed,
                "sampleRate": result.sample_rate,
                "settingsHash": result.settings_hash,
            }
        if method == "tts.close":
            if self._provider is not None:
                self._provider.close()
                self._provider = None
            return {"closed": True}
        raise ValueError(f"unknown method: {method}")

    def close(self) -> None:
        if self._provider is not None:
            self._provider.close()
            self._provider = None


def handle(method: str, params: dict) -> dict:
    """Route a single RPC method."""
    return _WORKER.handle(method, params)


def error_payload(exc: Exception) -> dict:
    """Build a JSON-RPC error object with a stable ``kind`` for known errors."""
    payload: dict[str, Any] = {"code": -32601, "message": str(exc)}
    if isinstance(exc, TtsError):
        payload["kind"] = exc.kind
    elif isinstance(exc, FileNotFoundError):
        payload["kind"] = "missing-file"
    else:
        payload["kind"] = ""
    return payload


_WORKER = Worker()


def run() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = parse_request(line)
            result = handle(request.method, request.params)
            response = RpcResponse(id=request.id, result=result)
        except Exception as exc:
            response = RpcResponse(
                id=getattr(exc, "id", -1),
                error=error_payload(exc),
            )
        sys.stdout.write(encode_response(response) + "\n")
        sys.stdout.flush()


def main() -> None:
    try:
        if hasattr(sys.stdin, "reconfigure"):
            sys.stdin.reconfigure(encoding="utf-8", errors="replace")
        if hasattr(sys.stdout, "reconfigure"):
            sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        if hasattr(sys.stderr, "reconfigure"):
            sys.stderr.reconfigure(encoding="utf-8", errors="replace")
        run()
        _WORKER.close()
    except (BrokenPipeError, KeyboardInterrupt):
        pass


if __name__ == "__main__":
    main()
