# DubFlow Project Schema

Status: Implemented (Phase 2). `schemaVersion: 1` is the current schema and
the migration boundary lives in `desktop/src-tauri/src/domain/project.rs`.

โปรเจกต์เป็นโฟลเดอร์ `.dubflow` ประกอบด้วย `project.json` ที่ versioned,
processed reference, raw takes และ cache

## project.json (v1)

```json
{
  "schemaVersion": 1,
  "name": "ตัวอย่างโปรเจกต์",
  "createdAt": "2026-08-10T00:00:00Z",
  "video": {
    "path": "C:/videos/input.mp4",
    "relinkKey": "sha256-of-original-path"
  },
  "srt": {
    "path": "C:/videos/input.srt",
    "encoding": "utf-8"
  },
  "reference": {
    "source": "video-segment",
    "videoPath": "C:/videos/input.mp4",
    "startMs": 12000,
    "endMs": 15000,
    "transcript": "สวัสดีครับ"
  },
  "cues": [
    {
      "id": "cue-001",
      "index": 1,
      "text": "สวัสดีครับ",
      "srtStartMs": 1000,
      "srtEndMs": 3000,
      "status": "Not Generated",
      "selectedTakeId": null,
      "takes": []
    }
  ]
}
```

## Raw Take (immutable)

```json
{
  "takeId": "take-001",
  "cueId": "cue-001",
  "provider": "jaitts-f5tts",
  "providerVersion": "1.1.22",
  "seed": 12345,
  "durationMs": 2100,
  "settingsHash": "sha256-of-settings",
  "audioPath": "takes/take-001.wav"
}
```

## Rules

- `schemaVersion` เปลี่ยนเมื่อโครงสร้างไม่ backward compatible; migration
  boundary อยู่ที่ deserialization layer: ระบบ reject version ต่ำกว่า 1 หรือสูงกว่า 1
- Raw take ห้ามแก้ไขหลังสร้าง; การเลือก take เก็บใน project state
- Path เก็บเป็น absolute แต่มี `relinkKey` สำหรับ relink เมื่อไฟล์ย้ายที่
- JSON ใช้ camelCase ตามตัวอย่างด้านบน; `createdAt` เป็น ISO-8601 UTC
- Timeline Solver คำนวณภายในด้วย integer samples ที่ 48 kHz (`samples = ms * 48`)
  เพื่อป้องกัน float drift; `renderStartMs/renderEndMs` เป็นค่าที่แปลงจาก samples
- `Cue` และ `Take` ใช้ camelCase fields และ optional fields (`selectedTakeId`,
  `takes`, `status`) มี default เมื่ออ่าน JSON ที่ไม่มี field นั้น
