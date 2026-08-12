from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class MediaInfo:
    duration_ms: int
    width: int
    height: int
    has_audio: bool


def probe_media(path: str, ffprobe: str = "ffprobe") -> MediaInfo:
    try:
        result = subprocess.run([ffprobe, "-v", "error", "-show_streams", "-show_format", "-of", "json", path], capture_output=True, text=True, check=False)
    except OSError as exc:
        raise RuntimeError(f"เรียก FFprobe ไม่ได้: {exc}") from exc
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "อ่านข้อมูลวิดีโอไม่ได้")
    try:
        data = json.loads(result.stdout)
        streams = data.get("streams", [])
        video = next((stream for stream in streams if stream.get("codec_type") == "video"), {})
        return MediaInfo(round(float(data.get("format", {}).get("duration", 0)) * 1000), int(video.get("width", 0)), int(video.get("height", 0)), any(stream.get("codec_type") == "audio" for stream in streams))
    except (TypeError, ValueError, json.JSONDecodeError) as exc:
        raise RuntimeError("FFprobe ส่งข้อมูล media ที่ไม่ถูกต้อง") from exc
