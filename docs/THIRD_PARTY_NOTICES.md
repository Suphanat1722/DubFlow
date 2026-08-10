# DubFlow Third-Party Notices

DubFlow is a desktop application that invokes third-party tools as
subprocesses (not linked into the binary). This document covers runtime
and media components that must be distributed or acknowledged alongside
the app.

## FFmpeg (LGPL-2.1-or-later build)

DubFlow uses `ffmpeg` and `ffprobe` as subprocesses for:
- Media analysis (duration, audio stream probes via ffprobe)
- Reference segment extraction, waveform peaks
- Pitch-preserving time-stretch (rubberband filter)
- Loudness normalization (loudnorm, EBU R128)
- Voice master assembly, Replace/Mix/Voice Track export

FFmpeg distributed with DubFlow uses an LGPL-compatible configuration
that includes GPL-licensed encoders (libx264). Before release (Phase 6):

1. Choose a GPL or LGPL-only build based on distribution policy
2. For GPL builds: include the full FFmpeg license text and source offer
3. For LGPL-only builds: remove GPL encoders, let users install x264
4. Display this notice in the installer and About dialog
5. Record the FFmpeg version and checksum in the release manifest

## Python runtime / PyTorch / CUDA (Phase 6)

The Python runtime, PyTorch wheels, and model weights are downloaded on
first use and stored in `runtime/` and `models/` directories (not in Git).
See each component's license before redistribution. JaiTTS model weights
are CC-BY-NC-4.0 and require acceptance before download.

## License matrix (MVP)

| Component | License | Notes |
|---|---|---|
| DubFlow source code | MIT | Does not cover third-party |
| FFmpeg (distributed) | LGPL-2.1+ or GPL | Must comply with copyleft terms |
| JaiTTS weights | CC-BY-NC-4.0 | Non-commercial only |
| PyTorch / torchaudio | BSD-3 | See individual PyTorch licenses |
| CUDA runtime | NVIDIA EULA | Downloaded via NVIDIA installer |
