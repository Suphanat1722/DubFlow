import { useMemo } from "react";
import { t, formatMs, formatSpeed, statusLabel } from "../lib/i18n";
import type { Cue, SolvedCue } from "../lib/types";

/**
 * Read-only timeline visualization. Cues are drawn as blocks positioned by
 * their solved render start/end. Hovering shows duration, speed and status.
 * The user cannot drag anything here (per MVP scope).
 */
export function TimelineBar({
  cues,
  solved,
  videoDurationMs,
  selectedCueId,
  onSelectCue,
}: {
  cues: Cue[];
  solved: SolvedCue[];
  videoDurationMs: number | null;
  selectedCueId: string | null;
  onSelectCue: (cueId: string) => void;
}) {
  const solvedById = useMemo(() => {
    const m = new Map<string, SolvedCue>();
    for (const s of solved) m.set(s.cueId, s);
    return m;
  }, [solved]);

  const endMs = videoDurationMs ?? Math.max(...cues.map((c) => c.srtEndMs), 1);
  const span = Math.max(endMs, 1);

  return (
    <div className="timeline-bar">
      <div
        className="timeline-track"
        style={{ position: "relative", height: 44 }}
        role="presentation"
      >
        {cues.map((cue) => {
          const s = solvedById.get(cue.id);
          if (!s) return null;
          const leftPct = (s.renderStartMs / span) * 100;
          const widthPct = Math.max(
            ((s.renderEndMs - s.renderStartMs) / span) * 100,
            0.4,
          );
          return (
            <div
              key={cue.id}
              className={`timeline-cue cue-${s.status.replace(/\s+/g, "-")} ${
                selectedCueId === cue.id ? "selected" : ""
              }`}
              style={{
                left: `${leftPct}%`,
                width: `${widthPct}%`,
              }}
              title={`${t("cueIndex")} ${cue.index} — ${statusLabel(
                s.status,
              )}\n${t("speed")}: ${formatSpeed(s.speed)}`}
              onClick={() => onSelectCue(cue.id)}
            >
              <span className="timeline-cue-index">{cue.index}</span>
            </div>
          );
        })}
        <div className="timeline-ruler" aria-hidden>
          {[0, 0.25, 0.5, 0.75, 1].map((f) => (
            <span key={f} style={{ left: `${f * 100}%` }}>
              {formatMs(Math.round(span * f))}
            </span>
          ))}
        </div>
      </div>
      <div className="timeline-legend">
        <span className="legend-dot dot-Ready" /> {t("ready")}
        <span className="legend-dot dot-Adjusted" /> {t("adjusted")}
        <span className="legend-dot dot-Too-Long" /> {t("tooLong")}
        <span className="legend-dot dot-Not-Generated" /> {t("notGenerated")}
      </div>
    </div>
  );
}
