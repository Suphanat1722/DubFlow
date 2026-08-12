from __future__ import annotations

import json
import re
import shutil
from datetime import datetime, timezone
from pathlib import Path

from app.models import Project, Take


SCHEMA_VERSION = 1


class ProjectError(RuntimeError):
    pass


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _safe_name(name: str) -> str:
    safe = re.sub(r'[<>:"/\\|?*\x00-\x1f]', "_", name).strip(" .")
    if not safe:
        raise ProjectError("ชื่อโปรเจกต์ต้องมีอักขระที่ใช้เป็นชื่อโฟลเดอร์ได้")
    return safe


class ProjectRepository:
    def __init__(self, workspace_root: str | Path):
        self.workspace_root = Path(workspace_root).expanduser().resolve()

    def ensure_workspace(self) -> None:
        for folder in ("models", "cache", "projects"):
            (self.workspace_root / folder).mkdir(parents=True, exist_ok=True)

    def create(self, name: str) -> tuple[Project, Path]:
        self.ensure_workspace()
        project_dir = self.workspace_root / "projects" / _safe_name(name)
        if project_dir.exists():
            raise ProjectError(f"มีโปรเจกต์ชื่อ {name} อยู่แล้ว")
        for folder in ("voices", "cache", "export"):
            (project_dir / folder).mkdir(parents=True, exist_ok=True)
        now = _now()
        project = Project(SCHEMA_VERSION, name, now, now)
        self.save(project, project_dir)
        return project, project_dir

    def load(self, project_file: str | Path) -> tuple[Project, Path]:
        file_path = Path(project_file).resolve()
        try:
            data = json.loads(file_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise ProjectError(f"เปิดโปรเจกต์ไม่ได้: {exc}") from exc
        if data.get("schema_version") != SCHEMA_VERSION:
            raise ProjectError(f"ไม่รองรับ project schema {data.get('schema_version')}")
        return Project.from_dict(data), file_path.parent

    def save(self, project: Project, project_dir: str | Path) -> Path:
        project.updated_at = _now()
        destination = Path(project_dir) / "project.json"
        temporary = destination.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(project.to_dict(), ensure_ascii=False, indent=2), encoding="utf-8")
        temporary.replace(destination)
        return destination

    def add_take(
        self,
        project: Project,
        project_dir: str | Path,
        cue_id: str,
        generated_file: str | Path,
        duration_ms: int,
        provider: str,
        provider_version: str,
        seed: int,
    ) -> Take:
        cue = next((item for item in project.cues if item.id == cue_id), None)
        if cue is None:
            raise ProjectError(f"ไม่พบ subtitle {cue_id}")
        cue_dir = Path(project_dir) / "voices" / f"{cue.index:04d}"
        cue_dir.mkdir(parents=True, exist_ok=True)
        number = max((int(take.id.rsplit("-", 1)[-1]) for take in cue.takes), default=0) + 1
        take_id = f"take-{number:02d}"
        destination = cue_dir / f"{take_id}.wav"
        if destination.exists():
            raise ProjectError(f"ไฟล์ take มีอยู่แล้ว: {destination}")
        shutil.copy2(generated_file, destination)
        take = Take(take_id, destination.relative_to(project_dir).as_posix(), duration_ms, provider, provider_version, seed, _now())
        cue.takes.append(take)
        if not cue.lock_take or cue.selected_take_id is None:
            cue.selected_take_id = take.id
        cue.generated_duration = duration_ms
        return take
