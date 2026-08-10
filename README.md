# DubFlow

DubFlow เป็น Windows desktop application สำหรับสร้างเสียงพากย์ไทยจากไฟล์ SRT ด้วย AI
ทีละประโยค จัดความยาวและตำแหน่งเสียงให้สัมพันธ์กับ timeline ของวิดีโอ และ export
เป็นวิดีโอหรือ voice track โดยใช้ FFmpeg

## สถานะโครงการ

- Windows 10/11 เท่านั้น (Tauri 2 + React/TypeScript + Rust shell + Python 3.11 sidecar)
- MVP: หนึ่ง Reference Voice ต่อโปรเจกต์, input เฉพาะ MP4 H.264/AAC + SRT UTF-8
- Hardware acceptance target: NVIDIA GTX 1070 Ti (CC 6.1, VRAM 8 GB)
- Modern GPU (CC >= 7.5) ยังไม่เข้า supported matrix จนกว่าจะมี hardware evidence

## การติดตั้ง

1. ดาวน์โหลด installer จาก release (Windows x64)
2. รัน DubFlow ครั้งแรก — ตัวแอปตรวจสอบ GPU, ขอ license acceptance (CC BY-NC 4.0)
   และดาวน์โหลด Python runtime, PyTorch wheels และ model weights ไปที่
   `%APPDATA%\DubFlow\` (ใช้ครั้งเดียว หลังติดตั้งแล้วทำงาน offline ได้)
3. สร้างโปรเจกต์ เลือกวิดีโอ MP4 + SRT สร้าง Reference Voice แล้ว Generate

> ต้องการพื้นที่ว่างอย่างน้อย ~6 GB สำหรับ runtime + model และอินเทอร์เน็ตเฉพาะครั้งแรก

## เอกสาร

- `docs/DEVELOPMENT.md` — วิธี build/test จาก source
- `docs/TOOLCHAIN.md` — toolchain ที่ใช้
- `docs/THIRD_PARTY_NOTICES.md` — license ของ FFmpeg/Python/PyTorch/model
- `docs/RELEASE_CHECKLIST.md` — clean-machine release checklist (Phase 6)
- `docs/ACCEPTANCE_FIXTURES.md` — acceptance fixtures และ license ของไฟล์ทดสอบ

## งานใน scope ของ MVP

ดู `docs/PROJECT_CONTEXT.md` สำหรับขอบเขตและสิ่งที่ไม่อยู่ใน MVP
