# Third-party notices

DubFlow source code is licensed under the MIT License. Optional runtime
components and models keep their own licenses; the MIT License does not replace
those terms.

| Component | Purpose | License / terms |
| --- | --- | --- |
| JaiTTS-F5TTS model weights | Thai speech generation | CC BY-NC 4.0; non-commercial restriction applies |
| F5-TTS | Speech synthesis runtime | See the license shipped by the upstream project |
| OpenAI Whisper Base model | Optional local transcript verification | Apache 2.0 |
| PyTorch and torchaudio | Machine-learning runtime | BSD-style upstream licenses |
| PySide6 / Qt | Desktop interface | LGPL/GPL/commercial terms from Qt |
| FFmpeg / FFprobe | Audio and video processing | Depends on the FFmpeg build used by the user |

DubFlow does not commit model weights, a CUDA runtime, or FFmpeg binaries to
this repository. Review the applicable upstream terms before redistributing a
packaged build.
