import { t, formatMs, statusLabel } from "../lib/i18n";
import type { Cue } from "../lib/types";

export function SubtitleList({
  cues,
  selectedCueId,
  onSelectCue,
  onTextChange,
  onGenerateOne,
  onRegenerateOne,
  onPlay,
  busyCueIds,
}: {
  cues: Cue[];
  selectedCueId: string | null;
  onSelectCue: (cueId: string) => void;
  onTextChange: (cueId: string, text: string) => void;
  onGenerateOne: (cueId: string) => void;
  onRegenerateOne: (cueId: string) => void;
  onPlay: (cueId: string) => void;
  busyCueIds: Set<string>;
}) {
  return (
    <div className="subtitle-list">
      <div className="list-header">
        <span className="col col-index">{t("cueIndex")}</span>
        <span className="col col-text">{t("cueText")}</span>
        <span className="col col-srt">{t("srtTime")}</span>
        <span className="col col-status">{t("status")}</span>
        <span className="col col-actions">{t("actions")}</span>
      </div>
      {cues.length === 0 && (
        <div className="empty-state">{t("noCues")}</div>
      )}
      {cues.map((cue) => {
        const busy = busyCueIds.has(cue.id);
        return (
          <div
            key={cue.id}
            className={`cue-row ${selectedCueId === cue.id ? "selected" : ""}`}
            onClick={() => onSelectCue(cue.id)}
            data-testid={`cue-row-${cue.id}`}
          >
            <span className="col col-index">{cue.index}</span>
            <div className="col col-text">
              <textarea
                value={cue.text}
                onChange={(e) => onTextChange(cue.id, e.target.value)}
                onClick={(e) => e.stopPropagation()}
                rows={1}
                data-testid={`cue-text-${cue.id}`}
              />
            </div>
            <span className="col col-srt">
              {formatMs(cue.srtStartMs)} → {formatMs(cue.srtEndMs)}
            </span>
            <span className={`col col-status status-${cue.status.replace(/\s+/g, "-")}`}>
              {statusLabel(cue.status)}
            </span>
            <span className="col col-actions" onClick={(e) => e.stopPropagation()}>
              {busy ? (
                <span className="busy-label">{t("generating")}</span>
              ) : (
                <>
                  <button
                    className="icon-btn"
                    title={t("play")}
                    disabled={!cue.selectedTakeId}
                    onClick={() => onPlay(cue.id)}
                  >
                    ▶
                  </button>
                  {cue.selectedTakeId ? (
                    <button
                      className="icon-btn"
                      title={t("regenerate")}
                      onClick={() => onRegenerateOne(cue.id)}
                    >
                      ↻
                    </button>
                  ) : (
                    <button
                      className="icon-btn"
                      title={t("generate")}
                      onClick={() => onGenerateOne(cue.id)}
                    >
                      ＋
                    </button>
                  )}
                </>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
