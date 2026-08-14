# DubFlow

Desktop GUI สำหรับสร้างเสียงพากย์ภาษาไทยจาก SRT ด้วย JaiTTS-F5TTS แล้วจัดเสียงให้เข้ากับ timeline ของวิดีโอ โดยเก็บ SRT ต้นฉบับและ raw Take ทุกชุดไว้เสมอ

> **สถานะ:** เวอร์ชัน `0.1.5` สำหรับ Windows อยู่ในช่วง MVP

## สิ่งที่ทำได้

- สร้าง/เปิด/บันทึกโปรเจกต์แยกโฟลเดอร์
- Import วิดีโอและ SRT (UTF-8, UTF-16 และ CP874)
- ใช้ reference voice จากไฟล์เสียงหรือดึงช่วง 3–12 วินาทีจากวิดีโอ
- Generate ทีละ subtitle หรือทั้งรายการผ่าน TTS provider interface
- Resume งานทั้งชุดโดยสร้างเฉพาะรายการที่ยังไม่มี Take และข้ามรายการผิดพลาดเพื่อทำรายการถัดไป
- เก็บหลาย Take ต่อ subtitle โดยไม่ overwrite Take เดิม
- เก็บทั้ง Raw Take และไฟล์หลังประมวลผล พร้อมใช้ Whisper ตรวจ coverage/คำท้ายและลอง seed ใหม่เมื่ออาจพูดไม่ครบ
- ตรวจ hard edge ที่ปลายเสียงและเติม release-tail โดยไม่แก้ไข Raw Take
- Trim silence, time stretch แบบไม่เปลี่ยน pitch, normalize และ fade ด้วย FFmpeg
- Auto-fit และ bounded ripple โดยหยุดที่ lock, large gap หรือ video end
- Preview วิดีโอพร้อม subtitle, เล่น Take และดู timeline แบบย่อ
- Export voice-only, replace audio หรือ mix พร้อม basic ducking
- ตรวจ NVIDIA GPU/VRAM โดยแยก logic ออกจาก GUI
- เปลี่ยน Workspace Root และ max speed ได้

## โครงสร้างข้อมูล

Workspace ที่เลือกจะมี `models/`, `cache/` และ `projects/` ส่วนแต่ละโปรเจกต์มี `voices/`, `cache/`, `export/` และ `project.json` ไฟล์เสียงอยู่ใน `voices/0001/take-01.wav` ตามลำดับ

## เริ่มพัฒนา

ต้องใช้ Python 3.10–3.12 และ FFmpeg/FFprobe ใน PATH ตรวจเครื่องมือที่มีอยู่ก่อนติดตั้งเสมอ

```powershell
python -m venv .venv
.venv\Scripts\python -m pip install -e .
.venv\Scripts\dubflow.exe
```

JaiTTS เป็น dependency ขนาดใหญ่และต้องเลือก PyTorch/CUDA build ให้ตรงกับ GPU ชุดที่ตรวจผ่านกับ GTX 1070 Ti คือ Python 3.11 และ CUDA 12.6:

```powershell
.venv\Scripts\python -m pip install torch==2.11.0+cu126 torchaudio==2.11.0+cu126 --index-url https://download.pytorch.org/whl/cu126
.venv\Scripts\python -m pip install -e ".[jaitts]"
```

โปรแกรมโหลด revision ที่ล็อกไว้ของ `JTS-AI/JaiTTS-F5TTS` ครั้งแรกเมื่อสั่ง Generate และเก็บ checkpoint กับ Vocos ใต้ `models/` ของ Workspace ที่เลือก

### โมเดลตรวจคำพูด

การตรวจว่าพูดครบเป็นระบบเสริมที่ใช้ Whisper แบบ local และไม่ดาวน์โหลดอัตโนมัติ วางไฟล์ `openai/whisper-base` ที่ `models/asr/whisper-base` ใต้ Workspace หรือเลือกโฟลเดอร์ Whisper อื่นใน **ตั้งค่า → โมเดลตรวจคำพูด** โฟลเดอร์ต้องมี `model.safetensors`, tokenizer และ config ของโมเดล หากไม่พบโมเดล โปรแกรมยังสร้างเสียงได้แต่จะใช้เพียงการตรวจความยาวและปลายคลื่น

## Lightweight Setup

ตัว Setup ไม่รวม PyTorch, CUDA หรือโมเดล เพื่อให้ดาวน์โหลดและอัปเดต GUI ได้โดยไม่ต้องโหลดไฟล์ AI ซ้ำ หลังเปิดโปรแกรมให้เข้า **ตั้งค่า → AI Runtime** แล้วเลือกโฟลเดอร์ Python environment ที่ติดตั้ง `torch`, `torchaudio` และ `f5-tts` ไว้ (เช่น `.venv`) จากนั้นเปิด DubFlow ใหม่ โมเดลสร้างเสียงจะดาวน์โหลด revision ที่รองรับจาก Hugging Face อัตโนมัติไปยัง `models/` ใต้ Workspace เมื่อสร้างเสียงครั้งแรก ส่วนโมเดล Whisper เป็นตัวเลือกที่ผู้ใช้กำหนดตำแหน่งเอง

## License และการใช้งานโมเดล

ซอร์สโค้ด DubFlow เป็นโอเพนซอร์สภายใต้ [MIT License](LICENSE) แต่โมเดล JaiTTS-F5TTS และส่วนประกอบภายนอกมีเงื่อนไขของตนเอง โดยเฉพาะน้ำหนักโมเดล JaiTTS-F5TTS ที่ใช้ CC BY-NC 4.0 และมีข้อจำกัดการใช้งานเชิงพาณิชย์ ดูรายละเอียดที่ [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)

## ทดสอบ Core

ชุดทดสอบไม่ต้องใช้ PySide6, FFmpeg หรือ model:

```powershell
python -m unittest discover -s tests -v
```

## ข้อจำกัด MVP

- Timeline เป็น visualization และ auto solver ยังไม่ใช่ editor แบบลากหลาย track
- Lightweight Setup ต้องชี้ไปยัง Python Runtime ที่เตรียม JaiTTS/PyTorch ไว้แยกต่างหาก
- การย้าย source video/SRT หลังสร้างโปรเจกต์ต้อง Import ใหม่

## Build และ Installer

มี [dubflow.spec](dubflow.spec) สำหรับ PyInstaller และ [installer/DubFlow.iss](installer/DubFlow.iss) สำหรับ Inno Setup ตัว installer เปิดหน้าเลือก Install Directory เสมอและไม่บังคับ Drive C ส่วน Workspace เปลี่ยนแยกได้ใน Settings

```powershell
python -m pip install -e ".[jaitts,build]"
pyinstaller --clean --noconfirm dubflow.spec
iscc installer\DubFlow.iss
```

เครื่องมือ build เหล่านี้ไม่รวมอยู่ในเครื่องทุกเครื่อง จึงต้องตรวจของเดิมและให้ผู้ใช้ยืนยันก่อนติดตั้ง
