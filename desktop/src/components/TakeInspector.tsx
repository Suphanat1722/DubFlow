import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t, formatMs, formatSpeed, statusLabel } from "../lib/i18n";
import { WaveformPeaks } from "./WaveformPeaks";
import type { Cue, SolvedCue, PeakSegment, Take } from "../lib/types";

export function TakeInspector({
  cue,
  solved,
  onSelectTake,
  onDeleteTake,
  onGenerateOne,
  onPlay,
}: {
  cue: Cue | null;
  solved: SolvedCue | null;
  onSelectTake: (cueId: string, takeId: string) => void;
  onDeleteTake: (cueId: string, takeId: string) => void;
  onGenerateOne: (cueId: string) => void;
  onPlay: (cueId: string) => void;
}) {
  const [peaks, setPeaks] = useState<PeakSegment[] | null>(null);
  const [peaksLoading, setPeaksLoading] = useState(false);

  const loadPeaks = useCallback(
    async (cueId: string, takeId: string) => {
      setPeaksLoading(true);
      try {
        const result = await invoke<PeakSegment[]>("compute_peaks", {
          cueId,
          takeId,
        });
        setPeaks(result);
      } catch {
        setPeaks(null);
      } finally {
        setPeaksLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (!cue || !cue.selectedTakeId) {
      setPeaks(null);
      return;
    }
    loadPeaks(cue.id, cue.selectedTakeId);
  }, [cue?.id, cue?.selectedTakeId, loadPeaks]);

  if (!cue) {
    return (
      <div className="take-inspector empty">
        <p>{t("takeInspectorEmpty")}</p>
      </div>
    );
  }

  const selectedTake = cue.takes.find((t) => t.takeId === cue.selectedTakeId);

  return (
    <div className="take-inspector">
      <h3 className="section-title">
        {t("currentCue")}: {cue.index} - {cue.text.substring(0, 40)}
        {cue.text.length > 40 ? "…" : ""}
      </h3>

      <div className="inspector-waveform">
        <WaveformPeaks peaks={peaks} loading={peaksLoading} />
      </div>

      <div className="inspector-info">
        {solved && (
          <>
            <p>
              {t("renderTime")}: {formatMs(solved.renderStartMs)} →{" "}
              {formatMs(solved.renderEndMs)} ({formatSpeed(solved.speed)})
            </p>
            <p>
              {t("status")}: {statusLabel(solved.status)}
            </p>
          </>
        )}
        {selectedTake ? (
          <>
            <p>
              {t("duration")}: {formatMs(selectedTake.durationMs)}
            </p>
            <p>
              {t("seed")}: {selectedTake.seed}
            </p>
            <p>
              {t("provider")}: {selectedTake.provider} v{selectedTake.providerVersion}
            </p>
            <div className="inspector-actions">
              <button className="icon-btn" onClick={() => onPlay(cue.id)}>
                ▶ {t("play")}
              </button>
              <button
                className="icon-btn"
                onClick={() => onDeleteTake(cue.id, selectedTake.takeId)}
              >
                🗑 {t("deleteTake")}
              </button>
            </div>
          </>
        ) : (
          <p>{t("noTakeSelected")}</p>
        )}
      </div>

      {cue.takes.length > 0 && (
        <div className="take-list">
          <h4>{t("takeCount")}: {cue.takes.length}</h4>
          {cue.takes.map((take: Take) => (
            <div
              key={take.takeId}
              className={`take-item ${take.takeId === cue.selectedTakeId ? "selected" : ""}`}
            >
              <span className="take-item-id">{take.takeId}</span>
              <span className="take-item-dur">{formatMs(take.durationMs)}</span>
              <span className="take-item-seed">#{take.seed}</span>
              <button
                className="btn btn-sm"
                onClick={() => onSelectTake(cue.id, take.takeId)}
                disabled={take.takeId === cue.selectedTakeId}
              >
                {take.takeId === cue.selectedTakeId ? t("takeSelected") : t("selectTake")}
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="inspector-new-take">
        <button className="btn btn-primary" onClick={() => onGenerateOne(cue.id)}>
          {t("takeInspectorGenerate")}
        </button>
      </div>
    </div>
  );
}
