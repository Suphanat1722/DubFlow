/**
 * Thai-first localization layer. All user-visible strings live here so a
 * second locale can be added without touching component code.
 */

const th = {
  appTitle: "DubFlow",
  appSubtitle: "เครื่องมือสร้างเสียงพากย์ไทยจาก SRT",

  // Project setup
  setupTitle: "สร้างโปรเจกต์ใหม่",
  projectName: "ชื่อโปรเจกต์",
  projectNamePlaceholder: "เช่น พากย์วิดีโอ EP.1",
  videoFile: "ไฟล์วิดีโอ (MP4)",
  srtFile: "ไฟล์คำบรรยาย (SRT)",
  chooseVideo: "เลือกวิดีโอ",
  chooseSrt: "เลือก SRT",
  chooseFolder: "เลือกโฟลเดอร์",
  createProject: "สร้างโปรเจกต์",
  openProject: "เปิดโปรเจกต์ที่มีอยู่",
  openProjectDir: "เปิดโฟลเดอร์โปรเจกต์",
  projectFolder: "โฟลเดอร์สำหรับเก็บโปรเจกต์",
  videoNotSelected: "ยังไม่ได้เลือกไฟล์วิดีโอ",
  srtNotSelected: "ยังไม่ได้เลือกไฟล์ SRT",
  folderNotSelected: "ยังไม่ได้เลือกโฟลเดอร์",
  creating: "กำลังสร้างโปรเจกต์...",
  loading: "กำลังโหลด...",
  readyToStart: "เลือกวิดีโอและไฟล์ SRT เพื่อเริ่มต้น",
  projectCreated: "สร้างโปรเจกต์เรียบร้อย",
  projectOpened: "เปิดโปรเจกต์เรียบร้อย",

  // Runtime / GPU
  runtimeStatus: "สถานะ Runtime",
  workerNotRunning: "Worker ยังไม่เริ่มทำงาน",
  workerRunning: "Worker ทำงานอยู่",
  initializingTts: "กำลังโหลดโมเดลเสียง...",
  ttsReady: "โมเดลเสียงพร้อมใช้งาน",
  gpuCuda: "CUDA (GPU)",
  gpuCpu: "CPU",
  noGpu: "ไม่พบ GPU ที่รองรับ ฟัง/Export ยังใช้งานได้",
  initializeRuntime: "เริ่ม Runtime",
  runtimeReady: "Runtime พร้อมใช้งาน",

  // Reference
  referenceTitle: "เสียงอ้างอิง",
  referenceNone: "ยังไม่มีเสียงอ้างอิง",
  referenceFromVideo: "จากวิดีโอ",
  referenceFromFile: "จากไฟล์เสียง",
  referenceStart: "เริ่มต้น (มม.)",
  referenceEnd: "สิ้นสุด (มม.)",
  referenceTranscript: "บทเสียงอ้างอิง",
  referenceTranscriptPlaceholder: "พิมพ์บทที่ตรงกับเสียงในช่วงนี้",
  buildReference: "สร้างเสียงอ้างอิง",
  buildReferenceVideo: "สร้างจากช่วงวิดีโอ",
  buildReferenceExternal: "สร้างจากไฟล์เสียง",
  referenceDuration: "ความยาวอ้างอิง",
  referenceBuilt: "สร้างเสียงอ้างอิงเรียบร้อย",
  referencePickSegment: "เลือกช่วง 3-12 วินาทีจากวิดีโอ",
  externalAudio: "ไฟล์เสียงภายนอก",
  chooseAudio: "เลือกไฟล์เสียง",
  referenceRequired: "ต้องสร้างเสียงอ้างอิงก่อน Generate",

  // Editor / timeline
  editorTitle: "แก้ไขไทม์ไลน์",
  subtitleList: "รายการคำบรรยาย",
  timeline: "ไทม์ไลน์",
  videoPreview: "ตัวอย่างวิดีโอ",
  takeInspector: "รายละเอียด Take",
  cueIndex: "#",
  cueText: "ข้อความ",
  srtTime: "เวลา SRT",
  renderTime: "เวลาที่เล่นจริง",
  speed: "ความเร็ว",
  status: "สถานะ",
  actions: "การกระทำ",
  generate: "สร้างเสียง",
  regenerate: "สร้างใหม่",
  generateAll: "สร้างทั้งหมด",
  cancel: "ยกเลิก",
  play: "ฟัง",
  delete: "ลบ",
  select: "เลือก",
  save: "บันทึก",
  closeProject: "ปิดโปรเจกต์",
  rawDuration: "เสียงดิบ",
  renderDuration: "เสียงที่ปรับแล้ว",
  rippleOffset: "ตำแหน่งเริ่ม (Ripple)",
  tooLong: "เสียงยาวเกิน",
  error: "เกิดข้อผิดพลาด",
  notGenerated: "ยังไม่ได้สร้าง",
  generating: "กำลังสร้าง",
  ready: "พร้อม",
  adjusted: "ปรับแล้ว",
  noCues: "ไม่มีคำบรรยายในโปรเจกต์นี้",
  noTakeSelected: "ยังไม่ได้เลือก Take",
  takeCount: "Take ทั้งหมด",
  seed: "Seed",
  provider: "Provider",
  duration: "ความยาว",
  selectTake: "เลือก Take นี้",
  exportReady: "พร้อม Export",
  exportBlocked: "ยัง Export ไม่ได้ ยังมีงานค้าง",
  currentCue: "Cue ที่เลือก",
  playHint: "คลิกเพื่อฟังเสียง",
  dragHint: "ไทม์ไลน์แสดงผลเท่านั้น",
  noReference: "สร้างเสียงอ้างอิงก่อน แล้วค่อย Generate",
  generatingInProgress: "กำลังสร้างเสียง...",
  jobStarted: "กำลังสร้าง",
  jobCompleted: "สร้างเสร็จ",
  jobFailed: "สร้างไม่สำเร็จ",
  jobCancelled: "ยกเลิกแล้ว",
  jobQueued: "เข้าคิว",
  exportStateReady: "ทุก Cue พร้อม Export",
  exportStateBlocked: "Export ถูกบล็อก",
  exportBlockers: "สาเหตุ",
  videoDuration: "ความยาววิดีโอ",
  totalRender: "เสียงรวม",
  msUnit: "มม.",
  selectCueFirst: "เลือก Cue ก่อน",
  saving: "กำลังบันทึก",
  saved: "บันทึกแล้ว",
  reload: "โหลดใหม่",
  projectInfo: "ข้อมูลโปรเจกต์",
  srtInfo: "ไฟล์ SRT",
  videoInfo: "ไฟล์วิดีโอ",
  errorBanner: "ข้อผิดพลาด",
  emptyProject: "ยังไม่มีโปรเจกต์",
  unsavedChanges: "มีการเปลี่ยนแปลงที่ยังไม่บันทึก",
  allStatuses: "ทุกสถานะ",

  // Take inspector
  takeInspectorEmpty: "เลือก Cue ที่มี Take เพื่อดูรายละเอียด",
  takeInspectorGenerate: "สร้าง Take ใหม่สำหรับ Cue นี้",
  deleteTake: "ลบ Take นี้",
  takeSelected: "เลือกแล้ว",

  // Waveform
  waveformLabel: "รูปคลื่นเสียง",
  waveformLoading: "กำลังโหลดรูปคลื่น...",
  waveformEmpty: "ไม่มีเสียงให้แสดง",
  waveformError: "โหลดรูปคลื่นไม่สำเร็จ",
  // Export
  exportTitle: "Export",
  exportModeReplace: "แทนที่เสียงเดิม",
  exportModeMix: "ผสมเสียง",
  exportModeVoiceTrack: "เฉพาะเสียงพากย์",
  exportReplaceDesc: "เสียงวิดีโอเดิมถูกแทนที่ด้วยเสียงพากย์ AAC 192 kbps (วิดีโอไม่ถูกเข้ารหัสใหม่)",
  exportMixDesc: "เสียงวิดีโอเดิม (-12 dB) ผสมกับเสียงพากย์ (0 dB) พร้อม Limiter",
  exportVoiceTrackDesc: "เสียงพากย์ mono 48 kHz 24-bit PCM WAV",
  exportOriginalGain: "ระดับเสียงเดิม (dB)",
  exportChooseOutput: "เลือกที่จัดเก็บ",
  exportRunning: "กําลัง Export...",
  exportSuccess: "Export สําเร็จ",
  exportFailed: "Export ไม่สําเร็จ",
  exportBlockedReason: "สาเหตุ",
  exportNoGpu: "Export ใช้ได้แม้ไม่มี GPU",
  exportOutput: "ไฟล์ที่ส่งออก",
};

export type Locale = typeof th;

const messages: Record<string, Locale> = {
  th,
};

let currentLocale = "th";

export function setLocale(locale: string): void {
  if (messages[locale]) {
    currentLocale = locale;
  }
}

/** Look up a Thai message by key. */
export function t(key: keyof Locale): string {
  return messages[currentLocale]?.[key] ?? th[key];
}

/** Format milliseconds as a compact `m:ss.mmm` timecode. */
export function formatMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "0:00.000";
  const totalSec = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSec / 60);
  const seconds = totalSec % 60;
  const millis = ms % 1000;
  return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

/** Format a duration as `1.25x` or `1.00x`. */
export function formatSpeed(speed: number): string {
  return `${speed.toFixed(2)}x`;
}

/** Map a Rust CueStatus to a Thai label. */
export function statusLabel(status: string): string {
  switch (status) {
    case "Not Generated":
      return t("notGenerated");
    case "Generating":
      return t("generating");
    case "Ready":
      return t("ready");
    case "Adjusted":
      return t("adjusted");
    case "Too Long":
      return t("tooLong");
    case "Error":
      return t("error");
    default:
      return status;
  }
}
