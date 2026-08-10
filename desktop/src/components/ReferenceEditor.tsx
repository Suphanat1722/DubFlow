import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";
import type { Project, ReferenceData } from "../lib/types";

interface Props {
  project: Project;
  onReferenceChange: (ref: ReferenceData) => void;
  onError: (msg: string) => void;
  onMessage: (msg: string) => void;
}

export function ReferenceEditor({ project, onReferenceChange, onError, onMessage }: Props) {
  const [tab, setTab] = useState<"video" | "external">("video");
  const [startMs, setStartMs] = useState("");
  const [endMs, setEndMs] = useState("");
  const [transcript, setTranscript] = useState("");
  const [externalAudio, setExternalAudio] = useState("");
  const [busy, setBusy] = useState(false);

  const existing = project.reference;

  const pickAudio = async () => {
    const result = await open({
      title: t("chooseAudio"),
      multiple: false,
      filters: [
        { name: "Audio Files", extensions: ["wav", "mp3", "m4a", "flac"] },
      ],
    });
    if (typeof result === "string") setExternalAudio(result);
  };

  const buildVideo = async () => {
    const s = parseInt(startMs, 10);
    const e = parseInt(endMs, 10);
    if (isNaN(s) || isNaN(e) || e - s < 3000 || e - s > 12000) {
      onError("ต้องเลือกช่วง 3-12 วินาที");
      return;
    }
    setBusy(true);
    try {
      const result = await invoke<{ reference: ReferenceData; durationMs: number }>(
        "reference_build_video_segment",
        { startMs: s, endMs: e, transcript }
      );
      onReferenceChange(result.reference);
      onMessage(`${t("referenceBuilt")} (${result.durationMs}ms)`);
    } catch (err) {
      onError(typeof err === "string" ? err : String(err));
    } finally {
      setBusy(false);
    }
  };

  const buildExternal = async () => {
    if (!externalAudio) {
      onError("กรุณาเลือกไฟล์เสียง");
      return;
    }
    setBusy(true);
    try {
      const result = await invoke<{ reference: ReferenceData; durationMs: number }>(
        "reference_build_external",
        { audioPath: externalAudio, transcript }
      );
      onReferenceChange(result.reference);
      onMessage(`${t("referenceBuilt")} (${result.durationMs}ms)`);
    } catch (err) {
      onError(typeof err === "string" ? err : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="reference-editor">
      <h3 className="section-title">{t("referenceTitle")}</h3>

      {existing ? (
        <div className="reference-info">
          <p>
            {t("referenceDuration")}: {existing.transcript ? existing.transcript.substring(0, 60) : ""}
            {existing.transcript?.length > 60 ? "…" : ""}
          </p>
          <p className="ref-meta">
            {existing.source === "video-segment" ? t("referenceFromVideo") : t("referenceFromFile")}
          </p>
        </div>
      ) : (
        <>
          <div className="tab-bar">
            <button
              className={`tab ${tab === "video" ? "active" : ""}`}
              onClick={() => setTab("video")}
            >
              {t("referenceFromVideo")}
            </button>
            <button
              className={`tab ${tab === "external" ? "active" : ""}`}
              onClick={() => setTab("external")}
            >
              {t("referenceFromFile")}
            </button>
          </div>

          {tab === "video" && (
            <div className="ref-video-form">
              <label className="field">
                <span className="field-label">{t("referenceStart")}</span>
                <input
                  type="number"
                  value={startMs}
                  onChange={(e) => setStartMs(e.target.value)}
                  placeholder="3000"
                />
              </label>
              <label className="field">
                <span className="field-label">{t("referenceEnd")}</span>
                <input
                  type="number"
                  value={endMs}
                  onChange={(e) => setEndMs(e.target.value)}
                  placeholder="8000"
                />
              </label>
              <label className="field">
                <span className="field-label">{t("referenceTranscript")}</span>
                <textarea
                  value={transcript}
                  onChange={(e) => setTranscript(e.target.value)}
                  placeholder={t("referenceTranscriptPlaceholder")}
                  rows={2}
                />
              </label>
              <button
                className="btn btn-primary"
                onClick={buildVideo}
                disabled={busy}
              >
                {busy ? t("loading") : t("buildReferenceVideo")}
              </button>
            </div>
          )}

          {tab === "external" && (
            <div className="ref-external-form">
              <label className="field">
                <span className="field-label">{t("externalAudio")}</span>
                <div className="file-row">
                  <input readOnly value={externalAudio} placeholder={t("chooseAudio")} />
                  <button className="btn" onClick={pickAudio}>
                    {t("chooseAudio")}
                  </button>
                </div>
              </label>
              <label className="field">
                <span className="field-label">{t("referenceTranscript")}</span>
                <textarea
                  value={transcript}
                  onChange={(e) => setTranscript(e.target.value)}
                  placeholder={t("referenceTranscriptPlaceholder")}
                  rows={2}
                />
              </label>
              <button
                className="btn btn-primary"
                onClick={buildExternal}
                disabled={busy}
              >
                {busy ? t("loading") : t("buildReferenceExternal")}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
