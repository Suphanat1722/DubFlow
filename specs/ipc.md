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

## Methods ที่ Phase 0 รองรับ

| Method | Params | Result |
|---|---|---|
| `system.ping` | `{}` | `{"pong": true}` |

Method สำหรับ TTS, audio analysis และ FFmpeg จะถูกเพิ่มใน Phase 1 ขึ้นไปตาม
`specs/tts-provider.md`
