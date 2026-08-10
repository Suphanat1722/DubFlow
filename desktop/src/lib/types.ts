/** Shared DubFlow domain types mirrored from the Rust shell (camelCase). */

export type CueStatus =
  | "Not Generated"
  | "Generating"
  | "Ready"
  | "Adjusted"
  | "Too Long"
  | "Error";

export interface Take {
  takeId: string;
  cueId: string;
  provider: string;
  providerVersion: string;
  seed: number;
  durationMs: number;
  settingsHash: string;
  audioPath: string;
}

export interface Cue {
  id: string;
  index: number;
  text: string;
  srtStartMs: number;
  srtEndMs: number;
  status: CueStatus;
  selectedTakeId?: string | null;
  takes: Take[];
}

export interface ReferenceData {
  source: string;
  videoPath: string;
  startMs: number;
  endMs: number;
  externalAudioPath: string;
  transcript: string;
  processedAudioPath: string;
}

export interface VideoRef {
  path: string;
  relinkKey: string;
}

export interface SrtRef {
  path: string;
  encoding: string;
}

export interface Project {
  schemaVersion: number;
  name: string;
  createdAt: string;
  video: VideoRef;
  srt: SrtRef;
  reference?: ReferenceData | null;
  cues: Cue[];
  projectDir: string;
  dirty: boolean;
}

export interface SolvedCue {
  cueId: string;
  srtStartSample: number;
  srtEndSample: number;
  renderStartMs: number;
  renderEndMs: number;
  renderStartSample: number;
  renderEndSample: number;
  speed: number;
  status: CueStatus;
}

export interface SolverResult {
  cues: SolvedCue[];
  exportBlocked: boolean;
  totalRenderEndMs: number;
}

export interface JobEvent {
  type: "queued" | "started" | "completed" | "failed" | "cancelled";
  cueId: string;
  takeId?: string;
  durationMs?: number;
  error?: { code: number; message: string; kind: string };
}

/** Simple segment-level peak representation (0-1 range). */
export interface PeakSegment {
  min: number;
  max: number;
}

export type ExportMode = "replace" | "mix" | "voiceTrack";

export interface ExportValidation {
  exportBlocked: boolean;
  reasons: string[];
}

export interface ExportResult {
  samples: number;
  outputPath: string;
}
