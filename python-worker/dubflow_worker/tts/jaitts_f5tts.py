"""JaiTTS-F5TTS adapter.

JaiTTS publishes a F5-TTS checkpoint at ``JTS-AI/JaiTTS-F5TTS`` plus a Thai
vocabulary file. This adapter loads that checkpoint through ``f5_tts`` classes
without embedding any model-specific code in the worker core.
"""

from __future__ import annotations

import hashlib
import os
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import soundfile as sf
import torch
import torchaudio
from huggingface_hub import hf_hub_download

from f5_tts.infer.utils_infer import infer_process, load_vocoder, preprocess_ref_audio_text
from f5_tts.model.utils import seed_everything

from . import (
    ReferenceArtifact,
    ReferenceInput,
    SynthesisRequest,
    SynthesisResult,
    SynthesisSettings,
    TtsError,
    settings_hash,
)


PROVIDER_ID = "jaitts-f5tts"
PROVIDER_VERSION = "1.1.22"
MODEL_REPO = "JTS-AI/JaiTTS-F5TTS"
MODEL_FILE = "model.pt"
VOCAB_FILE = "vocab.txt"
TARGET_SAMPLE_RATE = 24000


def _torchaudio_load_soundfile(
    uri,
    frame_offset: int = 0,
    num_frames: int = -1,
    normalize: bool = True,
    channels_first: bool = True,
    format: str | None = None,
    buffer_size: int = 0,
    backend: str | None = None,
):
    """Minimal torchaudio.load replacement for WAV references.

    TorchAudio 2.9+ always routes through torchcodec, which needs a shared
    FFmpeg runtime. DubFlow references are WAV files, so soundfile covers the
    Phase 1 audio without adding that runtime dependency.
    """
    del normalize, format, buffer_size, backend
    stop = None if num_frames < 0 else frame_offset + num_frames
    data, sr = sf.read(
        uri,
        start=frame_offset,
        stop=stop,
        dtype="float32",
        always_2d=False,
    )
    if data.ndim == 1:
        data = data[None, :]
    else:
        data = data.T
    tensor = torch.from_numpy(np.ascontiguousarray(data))
    if not channels_first:
        tensor = tensor.T
    return tensor, int(sr)


# Patch before any inference call so f5_tts.infer_process can load WAVs.
torchaudio.load = _torchaudio_load_soundfile


def _sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _decode_duration_ms(path: str, sample_rate: int) -> int:
    info = sf.info(path)
    return round(info.frames / info.samplerate * 1000)


@dataclass
class JaittsF5TtsProvider:
    """Concrete JaiTTS adapter implementing the worker TTS contract."""

    id: str = PROVIDER_ID
    version: str = PROVIDER_VERSION

    def __post_init__(self) -> None:
        self._model: Any | None = None
        self._vocoder: Any | None = None
        self._device: str | None = None
        self._hf_cache_dir: str | None = None
        self._vocab_file: str | None = None

    @property
    def device(self) -> str | None:
        return self._device

    def initialize(self, options: dict[str, Any]) -> None:
        cache_dir = options.get("cache_dir")
        if cache_dir:
            os.makedirs(cache_dir, exist_ok=True)
        self._hf_cache_dir = cache_dir
        if torch.cuda.is_available():
            self._device = "cuda"
            torch.cuda.empty_cache()
        else:
            self._device = "cpu"

        repo = options.get("model_repo", MODEL_REPO)
        vocab_repo = options.get("vocab_repo", repo)
        self._vocab_file = hf_hub_download(
            repo_id=vocab_repo,
            filename=VOCAB_FILE,
            cache_dir=self._hf_cache_dir,
        )
        ckpt_file = options.get("ckpt_file") or hf_hub_download(
            repo_id=repo,
            filename=MODEL_FILE,
            cache_dir=self._hf_cache_dir,
        )

        from f5_tts.model import CFM
        from f5_tts.model.backbones.dit import DiT
        from f5_tts.model.utils import get_tokenizer

        vocab_char_map, vocab_size = get_tokenizer(self._vocab_file, "custom")
        model_cfg = {
            "dim": 1024,
            "depth": 22,
            "heads": 16,
            "ff_mult": 2,
            "text_dim": 512,
            "text_mask_padding": True,
            "qk_norm": None,
            "conv_layers": 4,
            "pe_attn_head": None,
            "attn_backend": "torch",
            "attn_mask_enabled": False,
            "checkpoint_activations": False,
        }
        self._model = CFM(
            transformer=DiT(**model_cfg, text_num_embeds=vocab_size, mel_dim=100),
            mel_spec_kwargs=dict(
                n_fft=1024,
                hop_length=256,
                win_length=1024,
                n_mel_channels=100,
                target_sample_rate=TARGET_SAMPLE_RATE,
                mel_spec_type="vocos",
            ),
            odeint_kwargs=dict(method="euler"),
            vocab_char_map=vocab_char_map,
        ).to(self._device)

        dtype = torch.float32
        self._model = self._model.to(dtype)
        checkpoint = torch.load(ckpt_file, map_location=self._device, weights_only=True)
        if isinstance(checkpoint, dict) and "ema_model_state_dict" in checkpoint:
            state = {
                k.replace("ema_model.", ""): v
                for k, v in checkpoint["ema_model_state_dict"].items()
                if k not in ["initted", "step"]
            }
            for key in ["mel_spec.mel_stft.mel_scale.fb", "mel_spec.mel_stft.spectrogram.window"]:
                state.pop(key, None)
        elif isinstance(checkpoint, dict) and "model_state_dict" in checkpoint:
            state = checkpoint["model_state_dict"]
        else:
            raise ValueError("JaiTTS checkpoint must contain ema_model_state_dict or model_state_dict")
        self._model.load_state_dict(state)
        self._model = self._model.eval().to(self._device)
        self._vocoder = load_vocoder("vocos", is_local=False, local_path="", device=self._device, hf_cache_dir=self._hf_cache_dir)
        torch.cuda.empty_cache()

    def preprocess_reference(self, ref: ReferenceInput) -> ReferenceArtifact:
        if not os.path.isfile(ref.audio_path):
            raise FileNotFoundError(ref.audio_path)
        ref_audio, ref_text = preprocess_ref_audio_text(ref.audio_path, ref.transcript, show_info=lambda *_: None)
        info = sf.info(ref_audio)
        return ReferenceArtifact(
            audio_path=ref_audio,
            transcript=ref_text,
            duration_ms=round(info.frames / info.samplerate * 1000),
            sample_rate=int(info.samplerate),
            sha256=_sha256_file(ref.audio_path),
        )

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        if self._model is None or self._vocoder is None:
            raise RuntimeError("provider not initialized")
        seed_everything(request.seed)
        settings = request.settings
        wav, sr, _ = infer_process(
            request.reference.audio_path,
            request.reference.transcript,
            request.text,
            self._model,
            self._vocoder,
            mel_spec_type="vocos",
            show_info=lambda *_: None,
            progress=None,
            target_rms=settings.target_rms,
            cross_fade_duration=0.15,
            nfe_step=settings.nfe_step,
            cfg_strength=settings.cfg_strength,
            sway_sampling_coef=settings.sway_sampling_coef,
            speed=settings.speed,
            fix_duration=None,
            device=self._device,
        )
        if wav is None:
            raise TtsError("no-audio", "synthesis produced no audio")

        out_dir = Path(request.output_dir) if request.output_dir else None
        if not out_dir:
            out_dir = Path(tempfile.mkdtemp(prefix="dubflow-jaitts-"))
        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / f"take-{request.seed}.wav"
        sf.write(str(out_path), wav, sr)

        return SynthesisResult(
            audio_path=str(out_path),
            duration_ms=_decode_duration_ms(str(out_path), sr),
            seed=request.seed,
            sample_rate=int(sr),
            settings_hash=settings_hash(settings),
        )

    def close(self) -> None:
        self._model = None
        self._vocoder = None
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
