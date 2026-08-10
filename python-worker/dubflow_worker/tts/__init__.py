"""TTS provider interfaces and adapters.

The worker core only depends on ``TtsProvider``; JaiTTS lives behind the
``JaittsF5TtsProvider`` adapter so UI and shell code never import model code.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Any, Protocol


@dataclass(frozen=True)
class ReferenceInput:
    audio_path: str
    transcript: str


@dataclass(frozen=True)
class ReferenceArtifact:
    audio_path: str
    transcript: str
    duration_ms: int
    sample_rate: int
    sha256: str


@dataclass(frozen=True)
class SynthesisSettings:
    nfe_step: int = 32
    cfg_strength: float = 2.0
    sway_sampling_coef: float = -1.0
    speed: float = 1.0
    target_rms: float = 0.1

    def normalized_json(self) -> dict[str, Any]:
        return {
            "nfe_step": self.nfe_step,
            "cfg_strength": self.cfg_strength,
            "sway_sampling_coef": self.sway_sampling_coef,
            "speed": self.speed,
            "target_rms": self.target_rms,
        }


def settings_hash(settings: SynthesisSettings) -> str:
    """SHA-256 of normalized settings, independent of key order."""
    raw = json.dumps(
        settings.normalized_json(), sort_keys=True, separators=(",", ":")
    ).encode()
    return hashlib.sha256(raw).hexdigest()


@dataclass(frozen=True)
class SynthesisRequest:
    reference: ReferenceArtifact
    text: str
    seed: int
    settings: SynthesisSettings = field(default_factory=SynthesisSettings)
    output_dir: str | None = None


@dataclass(frozen=True)
class SynthesisResult:
    audio_path: str
    duration_ms: int
    seed: int
    sample_rate: int
    settings_hash: str


class TtsProvider(Protocol):
    id: str
    version: str

    def initialize(self, options: dict[str, Any]) -> None: ...

    def preprocess_reference(self, ref: ReferenceInput) -> ReferenceArtifact: ...

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult: ...

    def close(self) -> None: ...
