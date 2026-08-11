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
    result = subprocess.run([ffprobe, "-v", "error", "-show_streams", "-show_format", "-of", "json", path], capture_output=True, text=True, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "อ่านข้อมูลวิดีโอไม่ได้")
    data = json.loads(result.stdout)
    video = next((stream for stream in data["streams"] if stream.get("codec_type") == "video"), {})
    return MediaInfo(round(float(data["format"].get("duration", 0)) * 1000), int(video.get("width", 0)), int(video.get("height", 0)), any(s.get("codec_type") == "audio" for s in data["streams"]))
