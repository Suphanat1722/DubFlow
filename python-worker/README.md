# DubFlow Python Worker

Python 3.11 sidecar ที่รับ versioned JSON-RPC ทาง stdin/stdout ใช้ใน Phase 1
ขึ้นไปสําหรับ TTS, audio analysis และงาน FFmpeg แบบ queue

## Development

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -r requirements.txt
python -m pytest
```

ไม่มีการดาวน์โหลด model weights หรือ runtime ในขั้นตอนนี้
