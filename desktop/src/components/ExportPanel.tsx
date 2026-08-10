import { useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";
import type { ExportMode, ExportValidation, ExportResult } from "../lib/types";

const MODES: ExportMode[] = ["replace", "mix", "voiceTrack"];

function modeLabel(mode: ExportMode): string {
  switch (mode) {
    case "replace":
      return t("exportModeReplace");
    case "mix":
      return t("exportModeMix");
    case "voiceTrack":
      return t("exportModeVoiceTrack");
  }
}

function modeDescription(mode: ExportMode): string {
  switch (mode) {
    case "replace":
      return t("exportReplaceDesc");
    case "mix":
      return t("exportMixDesc");
    case "voiceTrack":
      return t("exportVoiceTrackDesc");
  }
}

/**
 * Export panel with mode selection (Replace/Mix/Voice Track), pre-flight
 * validation via `export_validate`, and a blocking export command. The
 * FFmpeg pipeline is driven by the Rust shell, so no GPU is required.
 */
export function ExportPanel({
  projectDir,
  onMessage,
  onError,
}: {
  projectDir: string;
  onMessage: (msg: string) => void;
  onError: (err: string) => void;
}) {
  const [mode, setMode] = useState<ExportMode>("replace");
  const [gain, setGain] = useState(-12);
  const [validation, setValidation] = useState<ExportValidation | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ExportResult | null>(null);

  const refreshValidation = useMemo(
    () => async (selectedMode: ExportMode) => {
      try {
        const v = await invoke<ExportValidation>("export_validate", {
          mode: selectedMode,
        });
        setValidation(v);
      } catch {
        setValidation(null);
      }
    },
    [],
  );

  // Validate on mount so the panel shows blockers without waiting for a mode
  // change.
  useEffect(() => {
    refreshValidation(mode);
  }, [mode, refreshValidation]);

  const changeMode = async (next: ExportMode) => {
    setMode(next);
    setResult(null);
    await refreshValidation(next);
  };

  const chooseOutput = async (): Promise<string | null> => {
    const filters =
      mode === "voiceTrack"
        ? [{ name: "WAV Audio", extensions: ["wav"] }]
        : [{ name: "MP4 Video", extensions: ["mp4"] }];
    const defaultPath = `${projectDir}\\export-${mode}.${
      mode === "voiceTrack" ? "wav" : "mp4"
    }`;
    const picked = await save({
      title: t("exportChooseOutput"),
      defaultPath,
      filters,
    });
    return typeof picked === "string" && picked.length > 0 ? picked : null;
  };

  const handleExport = async () => {
    const outputPath = await chooseOutput();
    if (!outputPath) return;
    setBusy(true);
    setResult(null);
    try {
      const res = await invoke<ExportResult>("export_run", {
        mode,
        outputPath,
        originalGainDb: mode === "mix" ? gain : undefined,
      });
      setResult(res);
      onMessage(`${t("exportSuccess")}: ${res.outputPath}`);
    } catch (err) {
      onError(`${t("exportFailed")}: ${typeof err === "string" ? err : String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const blocked = validation?.exportBlocked ?? false;

  return (
    <div className="export-panel" data-testid="export-panel">
      <div className="section-title">{t("exportTitle")}</div>
      <div className="export-modes">
        {MODES.map((m) => (
          <button
            key={m}
            className={`btn export-mode ${mode === m ? "btn-primary" : ""}`}
            onClick={() => changeMode(m)}
            disabled={busy}
            data-testid={`export-mode-${m}`}
          >
            {modeLabel(m)}
          </button>
        ))}
      </div>
      <p className="export-desc">{modeDescription(mode)}</p>

      {mode === "mix" && (
        <label className="field">
          <span className="field-label">{t("exportOriginalGain")}</span>
          <input
            type="number"
            step={1}
            min={-60}
            max={0}
            value={gain}
            onChange={(e) => setGain(Number(e.target.value))}
            disabled={busy}
            data-testid="mix-gain"
          />
        </label>
      )}

      {blocked && validation && (
        <div className="export-blocked" data-testid="export-blocked">
          <strong>{t("exportBlocked")}</strong>
          <ul>
            {validation.reasons.map((r, i) => (
              <li key={i}>{r}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="export-actions">
        <button
          className="btn btn-primary"
          onClick={handleExport}
          disabled={busy || blocked}
          data-testid="export-run"
        >
          {busy ? t("exportRunning") : t("exportTitle")}
        </button>
        <span className="export-hint">{t("exportNoGpu")}</span>
      </div>

      {result && (
        <div className="export-result" data-testid="export-result">
          {t("exportOutput")}: {result.outputPath}
        </div>
      )}
    </div>
  );
}
