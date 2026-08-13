from __future__ import annotations

from dataclasses import asdict, dataclass, field
from enum import Enum
from typing import Any


class CueStatus(str, Enum):
    NOT_GENERATED = "Not Generated"
    GENERATING = "Generating"
    READY = "Ready"
    ADJUSTED = "Adjusted"
    NEEDS_REVIEW = "Needs Review"
    ERROR = "Error"
    LOCKED = "Locked"


@dataclass
class Take:
    id: str
    path: str
    duration_ms: int
    provider: str
    provider_version: str
    seed: int
    created_at: str

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "Take":
        return cls(**value)

@dataclass
class Cue:
    id: str
    index: int
    original_start: int
    original_end: int
    text: str
    resolved_start: int | None = None
    resolved_end: int | None = None
    generated_duration: int | None = None
    final_duration: int | None = None
    speed: float = 1.0
    timing_shift: int = 0
    status: str = CueStatus.NOT_GENERATED.value
    warnings: list[str] = field(default_factory=list)
    takes: list[Take] = field(default_factory=list)
    selected_take_id: str | None = None
    lock_take: bool = False
    lock_timing: bool = False

    @property
    def slot_duration(self) -> int:
        return self.original_end - self.original_start

    @property
    def selected_take(self) -> Take | None:
        return next((take for take in self.takes if take.id == self.selected_take_id), None)

    @property
    def needs_generation(self) -> bool:
        """A batch resume only processes cues that do not already have a usable take."""
        return self.selected_take is None

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "Cue":
        value = dict(value)
        value["takes"] = [Take.from_dict(item) for item in value.get("takes", [])]
        return cls(**value)


@dataclass
class ReferenceVoice:
    source: str = "external"
    original_path: str = ""
    processed_path: str = ""
    transcript: str = ""
    start_ms: int | None = None
    end_ms: int | None = None


@dataclass
class Project:
    schema_version: int
    name: str
    created_at: str
    updated_at: str
    video_path: str = ""
    srt_path: str = ""
    reference: ReferenceVoice = field(default_factory=ReferenceVoice)
    cues: list[Cue] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "Project":
        value = dict(value)
        value["reference"] = ReferenceVoice(**value.get("reference", {}))
        value["cues"] = [Cue.from_dict(item) for item in value.get("cues", [])]
        return cls(**value)
