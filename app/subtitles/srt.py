from __future__ import annotations

import re
from pathlib import Path

from app.models import Cue


class SrtError(ValueError):
    pass


_TIMING = re.compile(
    r"(?P<sh>\d{1,2}):(?P<sm>\d{2}):(?P<ss>\d{2})[,.](?P<sms>\d{3})\s*-->\s*"
    r"(?P<eh>\d{1,2}):(?P<em>\d{2}):(?P<es>\d{2})[,.](?P<ems>\d{3})"
)


def _milliseconds(hour: str, minute: str, second: str, millis: str) -> int:
    return ((int(hour) * 60 + int(minute)) * 60 + int(second)) * 1000 + int(millis)


def parse_srt(content: str) -> list[Cue]:
    normalized = content.replace("\r\n", "\n").replace("\r", "\n").lstrip("\ufeff")
    blocks = re.split(r"\n\s*\n", normalized.strip()) if normalized.strip() else []
    cues: list[Cue] = []
    for block_number, block in enumerate(blocks, 1):
        lines = block.splitlines()
        if not lines:
            continue
        timing_index = next((i for i, line in enumerate(lines[:2]) if "-->" in line), None)
        if timing_index is None:
            raise SrtError(f"บล็อกที่ {block_number} ไม่มีบรรทัดเวลา")
        match = _TIMING.fullmatch(lines[timing_index].strip())
        if not match:
            raise SrtError(f"รูปแบบเวลาไม่ถูกต้องในบล็อกที่ {block_number}")
        groups = match.groupdict()
        start = _milliseconds(groups["sh"], groups["sm"], groups["ss"], groups["sms"])
        end = _milliseconds(groups["eh"], groups["em"], groups["es"], groups["ems"])
        if end <= start:
            raise SrtError(f"เวลาสิ้นสุดต้องมากกว่าเวลาเริ่มในบล็อกที่ {block_number}")
        try:
            index = int(lines[0].strip()) if timing_index == 1 else block_number
        except ValueError:
            index = block_number
        text = "\n".join(lines[timing_index + 1 :]).strip()
        cues.append(
            Cue(
                id=f"cue-{index:04d}",
                index=index,
                original_start=start,
                original_end=end,
                resolved_start=start,
                resolved_end=end,
                text=text,
            )
        )
    return cues


def parse_srt_file(path: str | Path) -> list[Cue]:
    source = Path(path)
    raw = source.read_bytes()
    for encoding in ("utf-8-sig", "utf-8", "cp874", "utf-16"):
        try:
            return parse_srt(raw.decode(encoding))
        except UnicodeDecodeError:
            continue
    raise SrtError("ไม่สามารถตรวจหารหัสอักขระของไฟล์ SRT ได้")
