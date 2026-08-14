from __future__ import annotations

import re
from pathlib import Path


def has_active_tail(path: str | Path, window_ms: int = 100, threshold_db: float = -38.0) -> bool:
    """Return True when speech-level energy reaches the physical end of a WAV."""
    try:
        import numpy as np
        import soundfile as sf

        audio, sample_rate = sf.read(str(path), dtype="float32", always_2d=True)
        if sample_rate <= 0 or not len(audio):
            return False
        mono = audio.mean(axis=1)
        samples = min(len(mono), max(1, round(sample_rate * window_ms / 1000)))
        rms = float(np.sqrt(np.mean(np.square(mono[-samples:]))))
        level_db = 20 * np.log10(max(rms, 1e-12))
        return bool(level_db > threshold_db)
    except (ImportError, OSError, RuntimeError, ValueError):
        return False


def assess_take_quality(text: str, slot_duration_ms: int, raw_duration_ms: int, processed_duration_ms: int) -> list[str]:
    """Return conservative warnings for likely truncation without claiming ASR accuracy."""
    warnings: list[str] = []
    if raw_duration_ms > 0:
        removed_ms = raw_duration_ms - processed_duration_ms
        if removed_ms > 350 and processed_duration_ms < raw_duration_ms * 0.78:
            warnings.append(f"การตัด silence ลดเสียงจาก {raw_duration_ms}ms เหลือ {processed_duration_ms}ms")

    thai_characters = len(re.findall(r"[\u0E01-\u0E7F]", text))
    latin_words = len(re.findall(r"[A-Za-z]+", text))
    speaking_units = thai_characters + latin_words * 3
    text_minimum = min(1800, max(500, speaking_units * 80))
    short_for_text = speaking_units >= 6 and processed_duration_ms < text_minimum
    short_for_slot = speaking_units >= 8 and slot_duration_ms >= 1200 and processed_duration_ms < slot_duration_ms * 0.45
    if short_for_text or short_for_slot:
        warnings.append(
            f"เสียงสั้นผิดปกติ ({processed_duration_ms}ms) เมื่อเทียบกับข้อความและช่วงเวลา {slot_duration_ms}ms · อาจพูดไม่ครบ"
        )
    return warnings
