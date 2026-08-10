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

## Phase 6 pinned components

Every downloadable component is pinned by SHA-256 in `runtime-manifest.json`
at the repo root. The manifest is verified before any install completes.

| Component | Version | SHA-256 (prefix) | License |
|---|---|---|---|
| Python embeddable (Windows x64) | 3.11.9 | `009d6bf7…` | PSF |
| FFmpeg (gyan.dev essentials) | 9.0 | `e6b54767…` | LGPL-2.1+ / GPL build |
| PyTorch | 2.11.0+cu126 | via pip wheel | BSD-3 |
| torchaudio | 2.11.0+cu126 | via pip wheel | BSD-3 |
| JaiTTS-F5TTS `model.pt` | rev `50a5aa8` | `74a7b9fd…` | CC-BY-NC-4.0 |
| JaiTTS-F5TTS `vocab.txt` | rev `50a5aa8` | `5e953a2f…` | CC-BY-NC-4.0 |
| vocos-mel-24khz `pytorch_model.bin` | rev `0feb3fd` | `97ec976a…` | MIT |
| vocos-mel-24khz `config.yaml` | rev `0feb3fd` | `da903392…` | MIT |

### Python embeddable note

`python.org` publishes Windows embeddable builds only for the last non
security-only patch of each feature release. The last 3.11 embeddable is
**3.11.9** (2 Apr 2024). DubFlow pins Python **3.11.15** for the full runtime
(torch cu126 wheel compatibility), but uses the 3.11.9 embeddable
distribution as the bundled interpreter. Both are CPython 3.11; see
`docs/DECISIONS.md` D-042.

### FFmpeg distribution choice

The gyan.dev "essentials" build is LGPL-2.1+ and excludes GPL encoders such
as libx264. DubFlow's Replace/Mix export re-muxes the input video stream
without re-encoding (Phase 5 criteria), so libx264 is not required at
runtime. For videos the user already has, `-c:v copy` keeps the codec.
Before shipping, confirm the distributed build's `--enable-gpl` state and
record the exact build URL + checksum in the release manifest.

## License matrix (MVP)

| Component | License | Notes |
|---|---|---|
| DubFlow source code | MIT | Does not cover third-party |
| FFmpeg (distributed) | LGPL-2.1+ or GPL | Must comply with copyleft terms |
| JaiTTS weights | CC-BY-NC-4.0 | Non-commercial only |
| PyTorch / torchaudio | BSD-3 | See individual PyTorch licenses |
| CUDA runtime | NVIDIA EULA | Downloaded via NVIDIA installer |
