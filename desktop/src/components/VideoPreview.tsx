import { useEffect, useRef, useState } from "react";
import { assetUrl } from "../lib/assetUrl";
import { t, formatMs } from "../lib/i18n";

/**
 * HTML5 video preview backed by the Tauri asset protocol. `onSeek` lets the
 * parent move the playhead to a cue's solved render start.
 */
export function VideoPreview({
  videoPath,
  onSeek,
  durationMs,
}: {
  videoPath: string;
  onSeek?: (ms: number) => void;
  durationMs?: number | null;
}) {
  const ref = useRef<HTMLVideoElement>(null);
  const [currentTime, setCurrentTime] = useState(0);

  useEffect(() => {
    if (ref.current) {
      ref.current.load();
    }
  }, [videoPath]);

  return (
    <div className="video-preview">
      <video
        ref={ref}
        src={assetUrl(videoPath)}
        controls
        onTimeUpdate={(e) => setCurrentTime(e.currentTarget.currentTime * 1000)}
        data-testid="video-preview"
      />
      <div className="video-controls">
        <span className="video-time">{formatMs(currentTime)}</span>
        {onSeek && (
          <button className="btn btn-sm" onClick={() => onSeek(currentTime)}>
            {t("playHint")}
          </button>
        )}
        {durationMs != null && (
          <span className="video-time">{formatMs(durationMs)}</span>
        )}
      </div>
    </div>
  );
}
