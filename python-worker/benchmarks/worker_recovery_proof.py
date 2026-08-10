"""Worker-level error recovery proof through the JSON-RPC protocol."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


os.environ.setdefault("PYTHONIOENCODING", "utf-8")
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")


def _json_lines(stdout: str) -> list[dict]:
    result: list[dict] = []
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return result


def main() -> int:
    cwd = str(Path(__file__).resolve().parents[1])
    ref_wav = str(Path(cwd) / "benchmarks" / "ref_bench.wav")
    requests = [
        json.dumps(
            {
                "protocolVersion": 1,
                "id": 1,
                "method": "tts.initialize",
                "params": {"options": {}},
            }
        ),
        json.dumps(
            {
                "protocolVersion": 1,
                "id": 2,
                "method": "tts.preprocess_reference",
                "params": {
                    "audioPath": ref_wav,
                    "transcript": "นี่คือเสียงตัวอย่างสําหรับการทดสอบ",
                },
            }
        ),
        json.dumps(
            {
                "protocolVersion": 1,
                "id": 3,
                "method": "tts.synthesize",
                "params": {
                    "reference": {
                        "audioPath": ref_wav,
                        "transcript": "นี่คือเสียงตัวอย่างสําหรับการทดสอบ. ",
                        "durationMs": 4990,
                        "sampleRate": 24000,
                        "sha256": "952c44c95f618af94f923a43a0db60ce5388987e5d2302a67fe366e98b1e5d64",
                    },
                    "text": "ทดสอบการทํางานของ worker ผ่าน RPC",
                    "seed": 42,
                    "settings": {},
                    "outputDir": str(Path(cwd) / "benchmarks" / "results" / "rpc"),
                },
            }
        ),
        json.dumps(
            {
                "protocolVersion": 1,
                "id": 4,
                "method": "tts.close",
                "params": {},
            }
        ),
    ]
    proc = subprocess.run(
        [sys.executable, "-m", "dubflow_worker.worker"],
        input="\n".join(requests) + "\n",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        cwd=cwd,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr)
    outputs = _json_lines(proc.stdout)
    print(json.dumps(outputs, ensure_ascii=False, indent=2))
    assert len(outputs) == 4, f"expected 4 responses, got {len(outputs)}"
    assert "error" not in outputs[2], f"synthesize failed: {outputs[2]}"
    print("ALL PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
