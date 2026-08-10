# DubFlow Project Schema (Draft)

โปรเจกต์เป็นโฟลเดอร์ `.dubflow` ประกอบด้วย `project.json` ที่ versioned,
processed reference, raw takes และ cache

## project.json (draft v1)

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
  อยู่ใน Phase 2
- Raw take ห้ามแก้ไขหลังสร้าง; การเลือก take เก็บใน project state
- Path เก็บเป็น absolute แต่มี `relinkKey` สําหรับ relink เมื่อไฟล์ย้ายที่
- ยังเป็น draft จน Phase 2 เริ่ม implement persistence
