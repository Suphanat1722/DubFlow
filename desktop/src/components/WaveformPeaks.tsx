import { useEffect, useRef } from "react";
import { t } from "../lib/i18n";

/**
 * Simple waveform display using pre-computed peak segments.
 * Accepts an array of {min, max} values in the range [-1, 1].
 * The canvas renders the waveform without loading raw audio.
 */
export function WaveformPeaks({
  peaks,
  loading,
  color = "#4f8ef7",
  height = 48,
}: {
  peaks?: { min: number; max: number }[] | null;
  loading?: boolean;
  color?: string;
  height?: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !peaks || peaks.length === 0) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth * dpr;
    const h = height * dpr;
    canvas.width = w;
    canvas.height = h;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, canvas.clientWidth, height);

    const barW = Math.max(1, canvas.clientWidth / peaks.length);
    const mid = height / 2;

    ctx.fillStyle = color;
    for (let i = 0; i < peaks.length; i++) {
      const p = peaks[i];
      const x = i * barW;
      const maxH = Math.abs(p.max) * mid;
      const minH = Math.abs(p.min) * mid;
      if (maxH > 0) {
        ctx.fillRect(x, mid - maxH, barW - 0.5, maxH);
      }
      if (minH > 0) {
        ctx.fillRect(x, mid, barW - 0.5, minH);
      }
    }
  }, [peaks, color, height]);

  if (loading) {
    return <div className="waveform-placeholder">{t("waveformLoading")}</div>;
  }
  if (!peaks || peaks.length === 0) {
    return <div className="waveform-placeholder">{t("waveformEmpty")}</div>;
  }

  return (
    <canvas
      ref={canvasRef}
      className="waveform-canvas"
      style={{ width: "100%", height }}
      aria-label={t("waveformLabel")}
    />
  );
}
