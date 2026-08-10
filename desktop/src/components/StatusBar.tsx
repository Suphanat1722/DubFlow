import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";

export type RuntimeState =
  | { kind: "checking" }
  | { kind: "no-worker" }
  | { kind: "worker-running" }
  | { kind: "initializing-tts" }
  | { kind: "tts-ready"; device: string }
  | { kind: "error"; message: string };

export function StatusBar({
  runtimeState,
  onInitRuntime,
}: {
  runtimeState: RuntimeState;
  onInitRuntime: () => void;
}) {
  const statusText = () => {
    switch (runtimeState.kind) {
      case "checking":
        return t("loading");
      case "no-worker":
        return t("workerNotRunning");
      case "worker-running":
        return t("workerRunning");
      case "initializing-tts":
        return t("initializingTts");
      case "tts-ready":
        return `${t("ttsReady")} (${runtimeState.device})`;
      case "error":
        return `${t("errorBanner")}: ${runtimeState.message}`;
    }
  };

  return (
    <div className="status-bar">
      <span className="status-text">{statusText()}</span>
      {(runtimeState.kind === "no-worker" || runtimeState.kind === "worker-running") && (
        <button className="btn btn-sm" onClick={onInitRuntime}>
          {t("initializeRuntime")}
        </button>
      )}
    </div>
  );
}

/** Hook to manage the full runtime lifecycle: spawn → configure → init TTS. */
export function useRuntime(projectDir: string | null) {
  const [state, setState] = useState<RuntimeState>({ kind: "checking" });

  const initRuntime = async () => {
    try {
      setState({ kind: "checking" });
      const result = await invoke<string>("worker_spawn", {
        pythonPath: null,
        workerDir: null,
      });
      if (result !== "already-running" && result !== "spawned") {
        setState({
          kind: "error",
          message: `unexpected spawn result: ${result}`,
        });
        return;
      }
      setState({ kind: "worker-running" });

      // Configure output dir
      if (projectDir) {
        const takesDir = `${projectDir}\\takes`;
        await invoke("worker_configure", { outputDir: takesDir });
      }

      // Initialize TTS
      setState({ kind: "initializing-tts" });
      const initResult = await invoke<Record<string, unknown>>("tts_initialize", {
        options: {},
      });
      const device = (initResult?.device as string) || "unknown";
      setState({ kind: "tts-ready", device });
    } catch (err) {
      setState({
        kind: "error",
        message: typeof err === "string" ? err : String(err),
      });
    }
  };

  return { state, initRuntime };
}
