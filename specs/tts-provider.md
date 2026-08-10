# TTS Provider Contract (Draft)

Core app รู้จักเฉพาะ interface กลาง ต่อไปนี้; JaiTTS อยู่ใน adapter
`jaitts-f5tts` และไม่ถูก import โดยตรงจาก UI หรือ Rust shell

## Interface

```ts
interface TtsProvider {
  id: string; // "jaitts-f5tts"
  version: string;
  initialize(options: ProviderInitOptions): Promise<void>;
  preprocessReference(ref: ReferenceInput): Promise<ReferenceArtifact>;
  synthesize(request: SynthesisRequest): Promise<SynthesisResult>;
  close(): Promise<void>;
}

interface SynthesisRequest {
  referenceArtifact: ReferenceArtifact;
  text: string;
  seed: number;
  settings: SynthesisSettings;
}

interface SynthesisResult {
  audioPath: string;
  durationMs: number;
  seed: number;
}
```

## Rules

- Provider ต้อง deterministic เมื่อ `seed` และ settings เหมือนกัน
- `SynthesisResult.durationMs` มาจาก actual decoded audio ไม่ใช่ค่าประมาณ
- Provider ต้อง recover จาก error ระดับ sentence โดยไม่ crash worker
- Settings hash ใช้ SHA-256 ของ JSON settings ที่ normalized
- รายละเอียด method ของ worker อยู่ที่ Phase 1 spike
