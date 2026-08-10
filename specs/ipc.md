# DubFlow IPC Protocol

ทุกการสื่อสารระหว่าง Rust desktop shell กับ Python worker ใช้ JSON-RPC 2.0
แบบ line-delimited ทาง stdin/stdout โดยมีฟิลด์ `protocolVersion` เพิ่มเติม

## Envelope

Request:

```json
{"protocolVersion": 1, "id": 1, "method": "system.ping", "params": {}}
```

Response:

```json
{"id": 1, "result": {"pong": true}}
```

Error:

```json
{"id": 1, "error": {"code": -32601, "message": "unknown method"}}
```

## Rules

- `protocolVersion` เป็น integer; worker ปฏิเสธเวอร์ชันที่ไม่รองรับ
- `id` เป็น integer ที่เพิ่มขึ้นเรื่อย ๆ ต่อ connection
- `params` เป็น object เสมอ (อาจว่าง)
- `result` หรือ `error` อย่างใดอย่างหนึ่งเท่านั้นใน response
- Response ต้องเขียนกลับภายในบรรทัดเดียว และ flush ทันที

## Methods

### Phase 0

| Method | Params | Result |
|---|---|---|
| `system.ping` | `{}` | `{"pong": true}` |

### Phase 1

| Method | Params | Result |
|---|---|---|
| `tts.initialize` | `{"cacheDir"?, "modelRepo"?, ...}` | `{"provider", "version", "device"}` |
| `tts.preprocess_reference` | `{"audioPath", "transcript"}` | `{"audioPath", "transcript", "durationMs", "sampleRate", "sha256"}` |
| `tts.synthesize` | `{"reference", "text", "seed", "settings", "outputDir"?}` | `{"audioPath", "durationMs", "seed", "sampleRate", "settingsHash"}` |
| `tts.close` | `{}` | `{"closed": true}` |

### Phase 3

| Method | Params | Result |
|---|---|---|
| `worker.configure` | `{"outputDir"}` | `{"outputDir"}` |

`worker.configure` ต้องถูกเรียกก่อน `tts.synthesize` เสมอ เพื่อให้ Rust shell
ควบคุมตำแหน่งไฟล์ take output (worker ไม่รับ `outputDir` ต่อ request อีกต่อไป
ตั้งแต่ Phase 3; กำหนดครั้งเดียวต่อ process)

## Structured errors (Phase 3)

ทุก error response มี `error.kind` ซึ่งเป็นรหัสคงที่ที่ Rust shell ใช้แมป
เป็นข้อความผู้ใช้:

| kind | ความหมาย |
|---|---|
| `missing-file` | ไฟล์อ้างอิง/audio ไม่พบ |
| `no-audio` | TTS ไม่สร้างเสียง (empty output) |
| `not-initialized` | เรียก synthesis ก่อน initialize |
| `out-of-memory` | VRAM ไม่พอ |
| `cuda-error` | CUDA error |
| `spawn` / `pipe` / `timeout` / `disconnected` | shell-side errors |
| `media` | FFmpeg/FFprobe error จาก Rust shell |

Encoding: worker ใช้ UTF-8 สำหรับ stdin/stdout/stderr ผ่าน
`sys.stdin/stdout/stderr.reconfigure(encoding="utf-8")`; Rust shell ต้องส่ง
JSON เป็น UTF-8 เสมอ (ภาษาไทยเป็น default text)
