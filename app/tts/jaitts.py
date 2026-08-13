from __future__ import annotations

import sys
import types
from contextlib import contextmanager
from pathlib import Path
from typing import Any

from .base import GenerationRequest, GenerationResult, TTSProvider, TtsError


@contextmanager
def _f5_inference_imports():
    """Avoid loading Transformers' optional ASR pipeline in packaged builds.

    DubFlow always requires a reference transcript, so F5-TTS never calls its
    automatic transcription path. Providing a guarded placeholder keeps the
    inference helpers lightweight without changing the installed dependency.
    """
    import transformers

    # f5_tts.model exports its training-only Trainer at package import time.
    # The Trainer pulls datasets/wandb/accelerate into an inference build even
    # though DubFlow never trains models. A placeholder preserves the upstream
    # package import contract while keeping those tools out of the executable.
    added_trainer_placeholder = "f5_tts.model.trainer" not in sys.modules
    if added_trainer_placeholder:
        trainer_module = types.ModuleType("f5_tts.model.trainer")

        class Trainer:
            def __init__(self, *_args, **_kwargs):
                raise TtsError("DubFlow รองรับการสร้างเสียงเท่านั้น ไม่รองรับการฝึกโมเดล")

        trainer_module.Trainer = Trainer
        sys.modules["f5_tts.model.trainer"] = trainer_module

    def unavailable_asr_pipeline(*_args, **_kwargs):
        raise TtsError("DubFlow ต้องใช้ transcript ของเสียงอ้างอิงและไม่เปิดใช้ ASR อัตโนมัติ")

    original_pipeline = transformers.pipeline
    transformers.pipeline = unavailable_asr_pipeline
    try:
        yield
    finally:
        transformers.pipeline = original_pipeline
        if added_trainer_placeholder:
            sys.modules.pop("f5_tts.model.trainer", None)


class JaiTTSProvider(TTSProvider):
    """Lazy JaiTTS-F5TTS adapter. Heavy dependencies load only in a worker."""

    id = "jaitts-f5tts"
    version = "1.1.22"
    model_repo = "JTS-AI/JaiTTS-F5TTS"
    model_revision = "50a5aa8986df1e3882873834f689a05bcae06bcb"

    def __init__(self) -> None:
        self._model: Any | None = None
        self._vocoder: Any | None = None
        self._device = "cpu"

    def get_capabilities(self) -> dict[str, Any]:
        return {"context": False, "voice_cloning": True, "languages": ["th"], "sample_rate": 24000}

    def load(self, model_dir: Path, cache_dir: Path) -> None:
        try:
            import torch
            from huggingface_hub import hf_hub_download
            with _f5_inference_imports():
                from f5_tts.infer.utils_infer import load_vocoder
                from f5_tts.model import CFM
                from f5_tts.model.backbones.dit import DiT
                from f5_tts.model.utils import get_tokenizer
        except (ImportError, OSError) as exc:
            missing = getattr(exc, "name", None) or str(exc)
            raise TtsError(f"JaiTTS runtime ยังไม่พร้อม (โหลด {missing} ไม่สำเร็จ)") from exc

        model_dir.mkdir(parents=True, exist_ok=True)
        cache_dir.mkdir(parents=True, exist_ok=True)
        self._device = "cuda" if torch.cuda.is_available() else "cpu"
        vocab_file = hf_hub_download(repo_id=self.model_repo, revision=self.model_revision, filename="vocab.txt", cache_dir=str(model_dir))
        checkpoint_file = hf_hub_download(repo_id=self.model_repo, revision=self.model_revision, filename="model.pt", cache_dir=str(model_dir))
        vocab_map, vocab_size = get_tokenizer(vocab_file, "custom")
        config = {"dim": 1024, "depth": 22, "heads": 16, "ff_mult": 2, "text_dim": 512, "text_mask_padding": True, "qk_norm": None, "conv_layers": 4, "pe_attn_head": None, "attn_backend": "torch", "attn_mask_enabled": False, "checkpoint_activations": False}
        self._model = CFM(
            transformer=DiT(**config, text_num_embeds=vocab_size, mel_dim=100),
            mel_spec_kwargs={"n_fft": 1024, "hop_length": 256, "win_length": 1024, "n_mel_channels": 100, "target_sample_rate": 24000, "mel_spec_type": "vocos"},
            odeint_kwargs={"method": "euler"},
            vocab_char_map=vocab_map,
        ).to(self._device, dtype=torch.float32)
        checkpoint = torch.load(checkpoint_file, map_location=self._device, weights_only=True)
        if not isinstance(checkpoint, dict) or "ema_model_state_dict" not in checkpoint:
            raise TtsError("JaiTTS checkpoint มีรูปแบบที่ไม่รองรับ")
        state = {key.replace("ema_model.", ""): value for key, value in checkpoint["ema_model_state_dict"].items() if key not in ("initted", "step")}
        for key in ("mel_spec.mel_stft.mel_scale.fb", "mel_spec.mel_stft.spectrogram.window"):
            state.pop(key, None)
        self._model.load_state_dict(state)
        self._model.eval()
        self._vocoder = load_vocoder("vocos", is_local=False, local_path="", device=self._device, hf_cache_dir=str(model_dir))

    def unload(self) -> None:
        self._model = None
        self._vocoder = None
        try:
            import torch
            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        except ImportError:
            pass

    def prepare_reference(self, audio_path: Path, transcript: str) -> tuple[Path, str]:
        if not audio_path.is_file():
            raise TtsError(f"ไม่พบไฟล์เสียงอ้างอิง: {audio_path}")
        if not transcript.strip():
            raise TtsError("ต้องระบุ transcript ของเสียงอ้างอิง")
        return audio_path, transcript.strip()

    def generate(self, request: GenerationRequest) -> GenerationResult:
        if self._model is None or self._vocoder is None:
            raise TtsError("ยังไม่ได้โหลด JaiTTS model")
        try:
            import numpy as np
            import soundfile as sf
            import torch
            import torchaudio
            with _f5_inference_imports():
                from f5_tts.infer.utils_infer import infer_process, preprocess_ref_audio_text
                from f5_tts.model.utils import seed_everything
        except (ImportError, OSError) as exc:
            missing = getattr(exc, "name", None) or str(exc)
            raise TtsError(f"JaiTTS runtime ไม่ครบ (โหลด {missing} ไม่สำเร็จ)") from exc
        # TorchAudio 2.9 routes file loading through torchcodec. Reference files
        # are normalized WAV, so SoundFile is a smaller and more reliable path.
        def load_reference(uri, frame_offset=0, num_frames=-1, normalize=True, channels_first=True, format=None, buffer_size=0, backend=None):
            del normalize, format, buffer_size, backend
            stop = None if num_frames < 0 else frame_offset + num_frames
            data, sample_rate = sf.read(uri, start=frame_offset, stop=stop, dtype="float32", always_2d=False)
            data = data[None, :] if data.ndim == 1 else data.T
            tensor = torch.from_numpy(np.ascontiguousarray(data))
            return (tensor if channels_first else tensor.T), int(sample_rate)

        original_torchaudio_load = torchaudio.load
        try:
            torchaudio.load = load_reference
            seed_everything(request.seed)
            reference, reference_text = preprocess_ref_audio_text(str(request.reference_audio), request.reference_text, show_info=lambda *_: None)
            wav, sample_rate, _ = infer_process(reference, reference_text, request.text, self._model, self._vocoder, mel_spec_type="vocos", show_info=lambda *_: None, progress=None, target_rms=0.1, cross_fade_duration=0.15, nfe_step=32, cfg_strength=2.0, sway_sampling_coef=-1.0, speed=1.0, fix_duration=None, device=self._device)
        finally:
            torchaudio.load = original_torchaudio_load
        if wav is None:
            raise TtsError("โมเดลไม่ส่งคืนข้อมูลเสียง")
        request.output_path.parent.mkdir(parents=True, exist_ok=True)
        sf.write(str(request.output_path), wav, sample_rate)
        duration_ms = round(len(wav) / sample_rate * 1000)
        return GenerationResult(request.output_path, duration_ms, request.seed)
