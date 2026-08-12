from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path


@dataclass
class AppSettings:
    workspace_root: str
    runtime_root: str = ""
    ffmpeg_path: str = "ffmpeg"
    ffprobe_path: str = "ffprobe"
    max_speed: float = 1.25
    large_gap_ms: int = 2000


class SettingsStore:
    """Project-local bootstrap settings; workspace data stays under the chosen root."""

    def __init__(self, bootstrap_file: str | Path):
        self.bootstrap_file = Path(bootstrap_file)

    def load(self) -> AppSettings:
        default_workspace = str((self.bootstrap_file.parent.parent / "workspace").resolve())
        if not self.bootstrap_file.exists():
            return AppSettings(default_workspace)
        try:
            data = json.loads(self.bootstrap_file.read_text(encoding="utf-8"))
            known = {field: data[field] for field in AppSettings.__dataclass_fields__ if field in data}
            return AppSettings(**known)
        except (OSError, json.JSONDecodeError, TypeError):
            return AppSettings(default_workspace)

    def save(self, settings: AppSettings) -> None:
        self.bootstrap_file.parent.mkdir(parents=True, exist_ok=True)
        temp = self.bootstrap_file.with_suffix(".tmp")
        temp.write_text(json.dumps(asdict(settings), ensure_ascii=False, indent=2), encoding="utf-8")
        temp.replace(self.bootstrap_file)
