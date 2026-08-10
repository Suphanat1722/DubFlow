# DubFlow Acceptance Fixtures

ไฟล์ทดสอบสำหรับ clean-machine acceptance (Phase 6 exit criteria) และสิ่งที่
ต้องตรวจก่อนใช้ เพื่อให้แน่ใจว่าสิทธิ์การใช้งานถูกต้อง

## Fixtures ที่แนะนำ

### 1. วิดีโอทดสอบ MP4 (H.264/AAC)

- สร้างเองด้วย FFmpeg lavfi (ไม่มีลิขสิทธิ์ติดมา):

```powershell
ffmpeg -y -f lavfi -i testsrc=duration=120:size=1280x720:rate=30 `
  -f lavfi -i sine=frequency=440:duration=120 `
  -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest sample-120s.mp4
```

- ต้องตรวจ: codec h264, audio aac, duration 120s

### 2. SRT ทดสอบ (UTF-8, ภาษาไทย)

- สร้างเอง 20+ บรรทัด ครอบกรณี: บรรทัดสั้น, บรรทัดยาว, overlap, gap
- ต้องตรวจ: encoding UTF-8, ไม่มี BOM ซ่อน, index ต่อเนื่องหรือ gap ได้

### 3. เสียงอ้างอิง (reference)

- ใช้จากวิดีโอช่วง 3-12 วินาที หรือไฟล์เสียงที่สร้างเอง
- ต้องตรวจ: เสียงชัด, ไม่มีลิขสิทธิ์ติดมา, ความยาวอยู่ในช่วง

## สิทธิ์การใช้งาน fixtures

ห้าม commit ไฟล์ media ที่มีลิขสิทธิ์ (เพลง, วิดีโอ commercial, เสียงจาก
บุคคลจริงโดยไม่ได้รับอนุญาต) เข้า repository หรือ release archive

- Fixtures ที่สร้างด้วย `lavfi`/`sine`/`testsrc` เป็น public domain โดยตัว FFmpeg
  เอง ไม่ติด license
- ถ้าจำเป็นต้องใช้เสียงจริง ต้องเป็นเสียงที่ผู้ใช้ให้สิทธิ์หรือ license เปิด
  (เช่น CC0) และบันทึกแหล่งที่มาไว้ในรายงาน acceptance

## Acceptance run template

```text
OS build:        Windows 11 24H2 (build 26100) / Windows 10 22H2 (19045)
GPU:             NVIDIA GeForce GTX 1070 Ti (CC 6.1, VRAM 8 GB)
Driver:          <driver version>
FFmpeg:          9.0 essentials (sha256 e6b5...)
Python embed:    3.11.9 (sha256 009d...)
PyTorch:         2.11.0+cu126
Model:           JaiTTS-F5TTS 50a5aa8 + vocos 0feb3fd
Cues generated:  N
Exports:         Replace/Mix/VoiceTrack (ffprobe results attached)
```
