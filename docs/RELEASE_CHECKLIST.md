# DubFlow Release Checklist (Phase 6)

Checklist นี้คือ gate สำหรับ release candidate: ทุกข้อต้องผ่านพร้อมหลักฐานก่อน
ประกาศ release ไฟล์หลักฐาน (screenshot/ffprobe/log) เก็บไว้ในที่ไม่อยู่ใน Git
(เช่น `docs/phase-reports/` ที่ gitignore ไว้ หรือ storage ภายนอก)

## 1. Clean machine (Windows 10/11 + GTX 1070 Ti)

ใช้ VM หรือเครื่องสะอาดที่ไม่มี Python/CUDA/FFmpeg ติดตั้งไว้ล่วงหน้า

- [ ] ติดตั้ง DubFlow installer (NSIS/Tauri) สำเร็จ
- [ ] เปิดแอปครั้งแรก → ตรวจพบ GPU (nvidia-smi) และแสดง CC 6.1
- [ ] แสดง license acceptance (CC BY-NC 4.0) ก่อนดาวน์โหลดโมเดล
- [ ] หลังยอมรับ license → ดาวน์โหลด Python runtime + PyTorch wheels + model
- [ ] ทุกไฟล์ผ่าน checksum verify (manifest `runtime-manifest.json`)
- [ ] ติดตั้ง atomic: ไม่มีไฟล์ครึ่ง ๆ หลงเหลือเมื่อ interrupted
- [ ] ทำงาน offline หลังติดตั้งครั้งแรก (ปิด network แล้วเปิดแอปใหม่)

## 2. Core workflow (สร้าง project → generate → select → export)

- [ ] สร้าง project จาก MP4 H.264/AAC + SRT UTF-8
- [ ] สร้าง Reference Voice จากช่วงวิดีโอ 3-12 วินาที
- [ ] Generate All อย่างน้อย 20 cue บน GTX 1070 Ti (ไม่ OOM)
- [ ] Regenerate และเปลี่ยน Take
- [ ] Export Replace: video stream copy (ffprobe codec h264) + AAC 192k
- [ ] Export Mix: original -12 dB + AI 0 dB + limiter
- [ ] Export Voice Track: mono 48 kHz 24-bit PCM WAV
- [ ] Duration ตรงกับวิดีโอภายใน tolerance (Phase 5 criteria)

## 3. Error / recovery paths

- [ ] Corrupt download (checksum mismatch) → retry สำเร็จ
- [ ] Network ตัดกลางคัน → resume ต่อได้
- [ ] Disk full → แสดง error ชัดเจนและไม่ทำให้ state เสียหาย
- [ ] Unsupported GPU → เปิดโปรเจกต์/ฟัง/export ยังได้ (ไม่มี TTS)
- [ ] Interrupted install → เปิดแอปใหม่ recovery ต่อจากจุดเดิม

## 4. Licensing / compliance

- [ ] `docs/THIRD_PARTY_NOTICES.md` ตรงกับสิ่งที่แจกจริง
- [ ] FFmpeg license: บันทึกเวอร์ชัน + checksum ใน release manifest
- [ ] JaiTTS weights CC BY-NC 4.0 แสดงใน installer/About
- [ ] ไม่มี model weights/runtime/archive ใน Git
- [ ] `runtime-manifest.json` hash ตรงกับไฟล์ที่ใช้ build release

## 5. Build artifacts

- [ ] `tauri build` ผ่านบน Windows x64
- [ ] Installer เปิดติดตั้งได้บน Windows 10 และ Windows 11
- [ ] Modern GPU ถูกระบุเป็น optional/unverified ไม่เข้า supported matrix

## Evidence ที่ต้องบันทึก

- OS build + driver version
- `nvidia-smi` output
- เวอร์ชันและ hash ของทุก component (manifest)
- ผล `cargo test`, `npm run test`, `npm run build`
- ffprobe output ของทั้ง 3 export modes
