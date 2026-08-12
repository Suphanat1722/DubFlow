from __future__ import annotations

import json
import shutil
import subprocess
from enum import Enum
from pathlib import Path

from app.models import Project


class FfmpegError(RuntimeError):
    pass


class ExportMode(str, Enum):
    VOICE_ONLY = "voice"
    REPLACE_AUDIO = "replace"
    MIX = "mix"


class AudioPipeline:
    def __init__(self, ffmpeg: str = "ffmpeg", ffprobe: str = "ffprobe"):
        self.ffmpeg = ffmpeg
        self.ffprobe = ffprobe

    def _run(self, args: list[str]) -> None:
        try:
            result = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False)
        except OSError as exc:
            raise FfmpegError(f"เรียก FFmpeg ไม่ได้: {exc}") from exc
        if result.returncode:
            message = result.stderr.strip().splitlines()[-1] if result.stderr.strip() else "FFmpeg ทำงานไม่สำเร็จ"
            raise FfmpegError(message)

    def duration_ms(self, path: str | Path) -> int:
        try:
            result = subprocess.run(
                [self.ffprobe, "-v", "error", "-show_entries", "format=duration", "-of", "json", str(path)],
                capture_output=True,
                text=True,
                check=False,
            )
        except OSError as exc:
            raise FfmpegError(f"เรียก FFprobe ไม่ได้: {exc}") from exc
        if result.returncode:
            raise FfmpegError(result.stderr.strip() or "อ่านความยาว media ไม่ได้")
        try:
            return round(float(json.loads(result.stdout)["format"]["duration"]) * 1000)
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
            raise FfmpegError("FFprobe ส่งข้อมูลความยาว media ที่ไม่ถูกต้อง") from exc

    def prepare_reference(self, source: str | Path, output: str | Path, start_ms: int | None = None, end_ms: int | None = None) -> Path:
        output_path = Path(output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        args = [self.ffmpeg, "-y"]
        if start_ms is not None:
            args += ["-ss", f"{start_ms / 1000:.3f}"]
        args += ["-i", str(source)]
        if start_ms is not None and end_ms is not None:
            args += ["-t", f"{(end_ms - start_ms) / 1000:.3f}"]
        args += ["-vn", "-ac", "1", "-ar", "24000", "-c:a", "pcm_s16le", str(output_path)]
        self._run(args)
        return output_path

    def trim_and_fit(self, source: str | Path, output: str | Path, speed: float = 1.0) -> Path:
        filters = ["silenceremove=start_periods=1:start_duration=0.05:start_threshold=-45dB:stop_periods=-1:stop_duration=0.08:stop_threshold=-45dB"]
        if abs(speed - 1.0) > 0.001:
            filters.append(f"atempo={speed:.6f}")
        filters += ["afade=t=in:st=0:d=0.015", "loudnorm=I=-16:TP=-1.5:LRA=11"]
        self._run([self.ffmpeg, "-y", "-i", str(source), "-af", ",".join(filters), "-ar", "48000", str(output)])
        return Path(output)

    def render_voice_track(self, project: Project, project_dir: str | Path, output: str | Path) -> Path:
        inputs: list[str] = []
        filters: list[str] = []
        input_index = 0
        for cue in project.cues:
            take = cue.selected_take
            if take is None:
                continue
            source = Path(project_dir) / take.path
            delay = max(0, cue.resolved_start if cue.resolved_start is not None else cue.original_start)
            inputs += ["-i", str(source)]
            speed = cue.speed if cue.speed > 0 else 1.0
            chain = f"[{input_index}:a]"
            if abs(speed - 1.0) > 0.001:
                chain += f"atempo={speed:.6f},"
            chain += f"adelay={delay}|{delay}[a{input_index}]"
            filters.append(chain)
            input_index += 1
        if not filters:
            raise FfmpegError("ยังไม่มี Take ที่เลือกสำหรับ Export")
        labels = "".join(f"[a{i}]" for i in range(input_index))
        filters.append(f"{labels}amix=inputs={input_index}:duration=longest:normalize=0,loudnorm=I=-16:TP=-1.5:LRA=11[out]")
        self._run([self.ffmpeg, "-y", *inputs, "-filter_complex", ";".join(filters), "-map", "[out]", "-ar", "48000", str(output)])
        return Path(output)

    def export(self, project: Project, project_dir: str | Path, output: str | Path, mode: ExportMode, voice_volume: float = 1.0, original_volume: float = 0.35, ducking: bool = True) -> Path:
        output = Path(output)
        voice_temp = Path(project_dir) / "cache" / "voice-track.wav"
        self.render_voice_track(project, project_dir, voice_temp)
        if mode == ExportMode.VOICE_ONLY:
            shutil.copy2(voice_temp, output)
            return output
        if not project.video_path:
            raise FfmpegError("โปรเจกต์ยังไม่มีวิดีโอ")
        if mode == ExportMode.REPLACE_AUDIO:
            self._run([self.ffmpeg, "-y", "-i", project.video_path, "-i", str(voice_temp), "-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-shortest", str(output)])
        else:
            if ducking:
                graph = f"[0:a]volume={original_volume}[orig];[1:a]volume={voice_volume},asplit=2[side][voice];[orig][side]sidechaincompress=threshold=0.03:ratio=8:attack=20:release=350[ducked];[ducked][voice]amix=inputs=2:duration=first[out]"
            else:
                graph = f"[0:a]volume={original_volume}[orig];[1:a]volume={voice_volume}[voice];[orig][voice]amix=inputs=2:duration=first[out]"
            self._run([self.ffmpeg, "-y", "-i", project.video_path, "-i", str(voice_temp), "-filter_complex", graph, "-map", "0:v:0", "-map", "[out]", "-c:v", "copy", "-shortest", str(output)])
        return output
