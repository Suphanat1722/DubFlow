from __future__ import annotations

import re
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any


def _normalize_transcript(text: str) -> str:
    return "".join(re.findall(r"[0-9a-z\u0e01-\u0e7f]+", text.lower()))


@dataclass(frozen=True)
class TranscriptAssessment:
    transcript: str
    coverage: float
    suffix_similarity: float
    length_ratio: float
    complete: bool


def assess_transcript(expected: str, transcript: str) -> TranscriptAssessment:
    """Compare noisy Thai ASR by character coverage and approximate suffix."""
    target = _normalize_transcript(expected)
    actual = _normalize_transcript(transcript)
    if not target or not actual:
        return TranscriptAssessment(transcript.strip(), 0.0, 0.0, 0.0, False)

    matcher = SequenceMatcher(None, target, actual, autojunk=False)
    matched = sum(block.size for block in matcher.get_matching_blocks())
    coverage = matched / len(target)
    suffix_size = min(8, len(target))
    suffix_similarity = SequenceMatcher(
        None,
        target[-suffix_size:],
        actual[-suffix_size:],
        autojunk=False,
    ).ratio()
    length_ratio = len(actual) / len(target)
    # Whisper Base is intentionally treated as a conservative signal rather
    # than exact Thai spelling. Strong overall agreement plus a recognizable
    # ending, or moderate coverage with a strong ending, counts as complete.
    complete = (
        coverage >= 0.68 and suffix_similarity >= 0.45 and length_ratio >= 0.65
    ) or (
        coverage >= 0.58 and suffix_similarity >= 0.60 and length_ratio >= 0.70
    )
    return TranscriptAssessment(transcript.strip(), coverage, suffix_similarity, length_ratio, complete)


class TranscriptVerifier:
    """Lazy local Whisper verifier; model files are always user-managed."""

    def __init__(self) -> None:
        self._processor: Any | None = None
        self._model: Any | None = None
        self._torch: Any | None = None
        self._device = "cpu"
        self._model_dir: Path | None = None

    def load(self, model_dir: str | Path) -> bool:
        root = Path(model_dir)
        if self._model is not None and self._model_dir == root.resolve():
            return True
        if not (root / "model.safetensors").is_file():
            return False
        self.unload()
        try:
            import torch
            from transformers import AutoModelForSpeechSeq2Seq, AutoProcessor

            device = "cuda" if torch.cuda.is_available() else "cpu"
            dtype = torch.float16 if device == "cuda" else torch.float32
            self._processor = AutoProcessor.from_pretrained(root, local_files_only=True)
            self._model = AutoModelForSpeechSeq2Seq.from_pretrained(
                root,
                local_files_only=True,
                dtype=dtype,
            ).to(device)
            self._model.eval()
            self._torch = torch
            self._device = device
            self._model_dir = root.resolve()
            return True
        except (ImportError, OSError, RuntimeError, ValueError):
            self.unload()
            return False

    def verify(self, expected: str, audio_path: str | Path) -> TranscriptAssessment:
        if self._model is None or self._processor is None or self._torch is None:
            raise RuntimeError("ยังไม่ได้โหลดโมเดลตรวจคำพูด")
        import numpy as np
        import soundfile as sf

        audio, sample_rate = sf.read(str(audio_path), dtype="float32", always_2d=True)
        mono = audio.mean(axis=1)
        if sample_rate != 16000 and len(mono):
            target_length = max(1, round(len(mono) * 16000 / sample_rate))
            mono = np.interp(
                np.linspace(0, len(mono) - 1, target_length),
                np.arange(len(mono)),
                mono,
            ).astype("float32")
        features = self._processor(mono, sampling_rate=16000, return_tensors="pt").input_features
        dtype = self._torch.float16 if self._device == "cuda" else self._torch.float32
        features = features.to(self._device, dtype=dtype)
        with self._torch.inference_mode():
            token_ids = self._model.generate(features, language="th", task="transcribe")
        transcript = self._processor.batch_decode(
            token_ids,
            skip_special_tokens=True,
            clean_up_tokenization_spaces=False,
        )[0]
        return assess_transcript(expected, transcript)

    def unload(self) -> None:
        self._processor = None
        self._model = None
        self._model_dir = None
        try:
            if self._torch is not None and self._torch.cuda.is_available():
                self._torch.cuda.empty_cache()
        except (AttributeError, RuntimeError):
            pass
        self._torch = None
