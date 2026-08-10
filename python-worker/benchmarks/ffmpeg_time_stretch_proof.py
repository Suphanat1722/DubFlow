"""FFmpeg 1.25x pitch-preserving time-stretch proof.

Creates a 10s 440 Hz test tone, stretches it with ``atempo=1.25``, then
verifies output duration tolerance and pitch deviation <= 1% via
autocorrelation.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import numpy as np
import soundfile as sf


def _estimate_freq_hz(path: str) -> float:
    data, sr = sf.read(path, dtype="float32")
    if data.ndim > 1:
        data = data.mean(axis=1)
    x = data[int(sr * 0.5) : int(sr * 4.5)]
    x = x - x.mean()
    n = len(x)
    n2 = 1 << (n - 1).bit_length()
    spectrum = np.fft.rfft(x, n=n2)
    ac = np.fft.irfft(np.abs(spectrum) ** 2, n=n2)[:n]
    ac[0] = 0.0
    lag_min = int(sr / 1000)
    lag_max = int(sr / 50)
    peak = lag_min + int(np.argmax(ac[lag_min:lag_max]))
    return sr / peak


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="benchmarks", help="output directory")
    parser.add_argument("--ffmpeg", default="ffmpeg", help="ffmpeg executable")
    args = parser.parse_args()

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    source = out / "ffmpeg_test_440.wav"
    stretched = out / "ffmpeg_test_440_stretched.wav"

    sr = 48000
    dur = 10.0
    t = np.linspace(0, dur, int(sr * dur), endpoint=False)
    sf.write(str(source), np.sin(2 * np.pi * 440 * t).astype(np.float32), sr)

    proc = subprocess.run(
        [
            args.ffmpeg,
            "-hide_banner",
            "-y",
            "-i",
            str(source),
            "-filter:a",
            "atempo=1.25",
            "-ar",
            "48000",
            str(stretched),
        ],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr)

    info = sf.info(str(stretched))
    duration = info.frames / info.samplerate
    expected = dur / 1.25
    duration_tolerance_s = 0.05
    f0 = _estimate_freq_hz(str(source))
    f1 = _estimate_freq_hz(str(stretched))
    pitch_deviation = abs(f1 - f0) / f0 * 100.0

    report = {
        "sourceHz": f0,
        "stretchedHz": f1,
        "pitchDeviationPercent": pitch_deviation,
        "pitchPass": pitch_deviation <= 1.0,
        "sourceDurationSeconds": dur,
        "expectedStretchedDurationSeconds": expected,
        "actualStretchedDurationSeconds": duration,
        "durationToleranceSeconds": duration_tolerance_s,
        "durationPass": abs(duration - expected) <= duration_tolerance_s,
        "sampleRate": info.samplerate,
    }
    (out / "ffmpeg_time_stretch.json").write_text(
        json.dumps(report, indent=2), encoding="utf-8"
    )
    print(json.dumps(report, indent=2))
    return 0 if (report["pitchPass"] and report["durationPass"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
