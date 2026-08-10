import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";
import { assetUrl } from "../lib/assetUrl";
import { StatusBar, useRuntime } from "./StatusBar";
import { ReferenceEditor } from "./ReferenceEditor";
import { SubtitleList } from "./SubtitleList";
import { TimelineBar } from "./TimelineBar";
import { VideoPreview } from "./VideoPreview";
import { TakeInspector } from "./TakeInspector";
import { ExportPanel } from "./ExportPanel";
import type { Project, SolvedCue, SolverResult, ReferenceData, JobEvent } from "../lib/types";

export function Editor({
  project: initialProject,
  onClose,
}: {
  project: Project;
  onClose: () => void;
}) {
  const [project, setProject] = useState<Project>(initialProject);
  const [solved, setSolved] = useState<SolvedCue[]>([]);
  const [selectedCueId, setSelectedCueId] = useState<string | null>(null);
  const [videoDuration, setVideoDuration] = useState<number | null>(null);
  const [busyCueIds, setBusyCueIds] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [generatingAll, setGeneratingAll] = useState(false);
  const eventPollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const kickRef = useRef(false);

  const { state: runtimeState, initRuntime } = useRuntime(project.projectDir);

  // Probe video duration on mount
  useEffect(() => {
    (async () => {
      try {
        const dur = await invoke<number>("probe_video_duration", {
          videoPath: project.video.path,
        });
        setVideoDuration(dur);
      } catch {
        // Not critical
      }
    })();
  }, [project.video.path]);

  // Re-solve when cues change
  const refreshSolver = useCallback(async () => {
    try {
      const result = await invoke<SolverResult>("solve_timeline");
      setSolved(result.cues);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    refreshSolver();
  }, [project.cues, refreshSolver]);

  const refreshProject = useCallback(async () => {
    try {
      const proj = await invoke<Project | null>("project_get");
      if (proj) setProject(proj);
    } catch {
      // ignore
    }
  }, []);

  // Kick a single queued job. The event poll watches for completion and
  // kicks the next queued job automatically. This avoids a blocking
  // while-loop in the React event handler.
  const kick = useCallback(async () => {
    if (kickRef.current) return;
    kickRef.current = true;
    try {
      const ev = await invoke<JobEvent | null>("jobs_run_next");
      if (ev && (ev.type === "completed" || ev.type === "failed")) {
        await refreshProject();
      }
    } catch {
      // queue empty or error
    } finally {
      kickRef.current = false;
    }
  }, [refreshProject]);

  // Poll job events (for progress and completion). Declared after kick so
  // the effect dependencies are in scope.
  useEffect(() => {
    const poll = async () => {
      try {
        const events = await invoke<JobEvent[]>("job_drain_events");
        let shouldKick = false;
        for (const ev of events) {
          switch (ev.type) {
            case "started":
              setBusyCueIds((prev) => new Set(prev).add(ev.cueId));
              break;
            case "completed":
            case "failed":
            case "cancelled":
              setBusyCueIds((prev) => {
                const next = new Set(prev);
                next.delete(ev.cueId);
                return next;
              });
              // Refresh the project to get updated takes/status
              await refreshProject();
              if (ev.type === "completed" || ev.type === "failed") {
                shouldKick = true;
              }
              break;
            case "queued":
              shouldKick = true;
              break;
          }
        }
        if (shouldKick) {
          kick();
        }
      } catch {
        // ignore
      }
    };
    eventPollRef.current = setInterval(poll, 500);
    return () => {
      if (eventPollRef.current) clearInterval(eventPollRef.current);
    };
  }, [kick, refreshProject]);

  const handleGenerateOne = useCallback(
    async (cueId: string) => {
      try {
        await invoke("generate_one", { cueId });
        kick();
      } catch (err) {
        setError(typeof err === "string" ? err : String(err));
      }
    },
    [kick],
  );

  const handleRegenerateOne = useCallback(
    async (cueId: string) => {
      try {
        const seed = Math.floor(Math.random() * 1000000000) + 1;
        await invoke("regenerate_one", { cueId, seed });
        kick();
      } catch (err) {
        setError(typeof err === "string" ? err : String(err));
      }
    },
    [kick],
  );

  const handleGenerateAll = useCallback(async () => {
    try {
      setGeneratingAll(true);
      const count = await invoke<number>("generate_all");
      if (count > 0) {
        await kick();
      }
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setGeneratingAll(false);
    }
  }, [kick]);

  const handleSelectTake = useCallback(
    async (cueId: string, takeId: string) => {
      try {
        await invoke("take_select", { cueId, takeId });
        await refreshProject();
      } catch (err) {
        setError(typeof err === "string" ? err : String(err));
      }
    },
    [refreshProject],
  );

  const handleDeleteTake = useCallback(
    async (cueId: string, takeId: string) => {
      try {
        await invoke("take_delete", { cueId, takeId });
        await refreshProject();
      } catch (err) {
        setError(typeof err === "string" ? err : String(err));
      }
    },
    [refreshProject],
  );

  const handleTextChange = useCallback(
    async (cueId: string, text: string) => {
      try {
        await invoke("cue_update_text", { cueId, text });
        await refreshProject();
      } catch (err) {
        setError(typeof err === "string" ? err : String(err));
      }
    },
    [refreshProject],
  );

  const handlePlay = useCallback(
    async (cueId: string) => {
      // Find the cue and its selected take audio path
      const cue = project.cues.find((c) => c.id === cueId);
      if (!cue || !cue.selectedTakeId) return;
      const take = cue.takes.find((t) => t.takeId === cue.selectedTakeId);
      if (!take) return;
      const audioPath = project.projectDir + "\\" + take.audioPath;
      const url = assetUrl(audioPath);
      const audio = new Audio(url);
      audio.play().catch(() => undefined);
    },
    [project],
  );

  const handleCancel = useCallback(async () => {
    try {
      await invoke("job_cancel_after_current");
    } catch {
      // ignore
    }
  }, []);

  const handleSave = useCallback(async () => {
    try {
      await invoke("project_save");
      setMessage(t("saved"));
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    }
  }, []);

  const handleClose = useCallback(async () => {
    await invoke("project_close");
    onClose();
  }, [onClose]);

  const selectedCue = project.cues.find((c) => c.id === selectedCueId) ?? null;
  const selectedSolved = solved.find((s) => s.cueId === selectedCueId) ?? null;
  const exportBlocked = useMemo(
    () => solved.some((s) => s.status === "Not Generated" || s.status === "Error" || s.status === "Too Long"),
    [solved],
  );

  return (
    <div className="editor">
      <header className="editor-header">
        <div className="editor-title-row">
          <h2>{project.name}</h2>
          <span className="editor-file-info">
            {t("videoInfo")}: {project.video.path.split("\\").pop()}
          </span>
        </div>
        <div className="editor-toolbar">
          <button
            className={`btn ${generatingAll ? "btn-disabled" : "btn-primary"}`}
            onClick={handleGenerateAll}
            disabled={generatingAll || !project.reference}
            title={!project.reference ? t("referenceRequired") : undefined}
          >
            {generatingAll ? t("generatingInProgress") : t("generateAll")}
          </button>
          <button className="btn" onClick={handleCancel} disabled={!generatingAll && busyCueIds.size === 0}>
            {t("cancel")}
          </button>
          <button className="btn" onClick={handleSave}>
            {t("save")}
          </button>
          <button className="btn" onClick={handleClose}>
            {t("closeProject")}
          </button>
        </div>
        <div className="editor-status-summary">
          {exportBlocked ? (
            <span className="status-blocked">{t("exportStateBlocked")}</span>
          ) : (
            <span className="status-ready">{t("exportStateReady")}</span>
          )}
          {videoDuration != null && (
            <span className="video-duration">
              {t("videoDuration")}: {Math.round(videoDuration / 1000)}s
            </span>
          )}
        </div>
      </header>

      {error && (
        <div className="banner error" onClick={() => setError(null)}>
          {t("errorBanner")}: {error}
        </div>
      )}
      {message && (
        <div className="banner info" onClick={() => setMessage(null)}>
          {message}
        </div>
      )}

      <StatusBar runtimeState={runtimeState} onInitRuntime={initRuntime} />

      <div className="editor-body">
        <div className="editor-left">
          <VideoPreview videoPath={project.video.path} durationMs={videoDuration} />

          {!project.reference && (
            <ReferenceEditor
              project={project}
              onReferenceChange={(ref: ReferenceData) => {
                setProject((prev) => ({ ...prev, reference: ref }));
              }}
              onError={setError}
              onMessage={setMessage}
            />
          )}

          <TimelineBar
            cues={project.cues}
            solved={solved}
            videoDurationMs={videoDuration}
            selectedCueId={selectedCueId}
            onSelectCue={setSelectedCueId}
          />

          <SubtitleList
            cues={project.cues}
            selectedCueId={selectedCueId}
            onSelectCue={setSelectedCueId}
            onTextChange={handleTextChange}
            onGenerateOne={handleGenerateOne}
            onRegenerateOne={handleRegenerateOne}
            onPlay={handlePlay}
            busyCueIds={busyCueIds}
          />
        </div>

        <div className="editor-right">
          <TakeInspector
            cue={selectedCue}
            solved={selectedSolved}
            onSelectTake={handleSelectTake}
            onDeleteTake={handleDeleteTake}
            onGenerateOne={handleGenerateOne}
            onPlay={handlePlay}
          />
          <ExportPanel
            projectDir={project.projectDir}
            onMessage={setMessage}
            onError={setError}
          />
        </div>
      </div>
    </div>
  );
}
