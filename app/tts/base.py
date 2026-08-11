from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path
from typing import Any


class TtsError(RuntimeError):
    pass


@dataclass(frozen=True)
class GenerationRequest:
    text: str
    reference_audio: Path
    reference_text: str
    output_path: Path
    previous_text: str = ""
    next_text: str = ""
    seed: int = 42


@dataclass(frozen=True)
class GenerationResult:
    path: Path
    duration_ms: int
    seed: int


class TTSProvider(ABC):
    id = "base"
    version = "0"

    @abstractmethod
    def load(self, model_dir: Path, cache_dir: Path) -> None: ...

    @abstractmethod
    def unload(self) -> None: ...

    @abstractmethod
    def prepare_reference(self, audio_path: Path, transcript: str) -> tuple[Path, str]: ...

    @abstractmethod
    def generate(self, request: GenerationRequest) -> GenerationResult: ...

    @abstractmethod
    def get_capabilities(self) -> dict[str, Any]: ...
