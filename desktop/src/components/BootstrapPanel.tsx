import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";

export interface BootstrapCheckResult {
  state: BootstrapState;
  gpu: GpuInfo;
  needsRuntime: boolean;
  needsModels: boolean;
  licensesPending: ModelLicense[];
  spaceOk: boolean;
}

export interface BootstrapState {
  runtimeInstalled: boolean;
  modelsInstalled: boolean;
  licensesAccepted: string[];
  installedAt: string;
  pythonPath: string;
  hfCacheDir: string;
  ffmpegPath: string;
  bootstrapComplete: boolean;
}

export interface GpuInfo {
  present: boolean;
  name: string;
  computeCapability: string;
  vramBytes: number;
  cudaVersion: string;
  supported: boolean;
  modernCandidate: boolean;
}

export interface ModelLicense {
  modelId: string;
  name: string;
  licenseUrl: string;
  licenseText: string;
  requiresAcceptance: boolean;
}

export interface DownloadProgress {
  totalBytes: number;
  downloadedBytes: number;
  currentFile: string;
  status: string;
}

export type BootstrapPhase =
  | { kind: "checking" }
  | { kind: "ready"; check: BootstrapCheckResult }
  | { kind: "license"; license: ModelLicense }
  | { kind: "downloading"; progress: DownloadProgress }
  | { kind: "installing"; message: string }
  | { kind: "complete"; state: BootstrapState }
  | { kind: "error"; message: string };

export function BootstrapPanel({
  onComplete,
}: {
  onComplete: (state: BootstrapState) => void;
}) {
  const [phase, setPhase] = useState<BootstrapPhase>({ kind: "checking" });

  useEffect(() => {
    (async () => {
      try {
        const check = await invoke<BootstrapCheckResult>("bootstrap_check");
        if (check.state.bootstrapComplete) {
          setPhase({ kind: "complete", state: check.state });
          onComplete(check.state);
          return;
        }
        setPhase({ kind: "ready", check });
      } catch (err) {
        setPhase({
          kind: "error",
          message: typeof err === "string" ? err : String(err),
        });
      }
    })();
  }, [onComplete]);

  const handleAcceptLicense = useCallback(async (license: ModelLicense) => {
    try {
      await invoke("bootstrap_accept_license", { modelId: license.modelId });
      const check = await invoke<BootstrapCheckResult>("bootstrap_check");
      setPhase({ kind: "ready", check });
    } catch (err) {
      setPhase({
        kind: "error",
        message: typeof err === "string" ? err : String(err),
      });
    }
  }, []);

  const handleStartDownload = useCallback(async () => {
    setPhase({
      kind: "downloading",
      progress: { totalBytes: 0, downloadedBytes: 0, currentFile: "", status: "" },
    });
    try {
      await invoke("bootstrap_ensure_dirs");
      const poll = setInterval(async () => {
        try {
          const p = await invoke<DownloadProgress>("bootstrap_download_progress");
          setPhase({ kind: "downloading", progress: p });
        } catch { /* poll */ }
      }, 500);
      await invoke("bootstrap_run_install");
      clearInterval(poll);
      setPhase({ kind: "installing", message: t("bootstrapCompleting") });
      const check = await invoke<BootstrapCheckResult>("bootstrap_check");
      setPhase({ kind: "complete", state: check.state });
      onComplete(check.state);
    } catch (err) {
      setPhase({
        kind: "error",
        message: typeof err === "string" ? err : String(err),
      });
    }
  }, [onComplete]);

  switch (phase.kind) {
    case "checking":
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <p className="bootstrap-status">{t("loading")}</p>
          </div>
        </div>
      );
    case "ready": {
      const { check } = phase;
      if (check.licensesPending.length > 0) {
        return (
          <div className="bootstrap-panel">
            <div className="bootstrap-card">
              <h2>{t("bootstrapTitle")}</h2>
              <p className="bootstrap-subtitle">{t("bootstrapSubtitle")}</p>
              <div className="bootstrap-section">
                <h3>{t("bootstrapHardware")}</h3>
                {check.gpu.present ? (
                  <div className="bootstrap-info">
                    <p>{check.gpu.name} CC {check.gpu.computeCapability} VRAM {check.gpu.cudaVersion}</p>
                  </div>
                ) : (
                  <p>{t("bootstrapNoGpu")}</p>
                )}
              </div>
              <div className="bootstrap-section">
                <h3>{t("bootstrapLicense")}</h3>
                {check.licensesPending.map((lic) => (
                  <div key={lic.modelId} className="bootstrap-license-card">
                    <pre className="bootstrap-license-text">{lic.licenseText}</pre>
                    <button className="btn btn-primary" onClick={() => handleAcceptLicense(lic)}>
                      {t("bootstrapAcceptLicense")}
                    </button>
                  </div>
                ))}
              </div>
            </div>
          </div>
        );
      }
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <p className="bootstrap-subtitle">{t("bootstrapSubtitle")}</p>
            <div className="bootstrap-section">
              <h3>{t("bootstrapHardware")}</h3>
              {check.gpu.present ? (
                <p className="status-ready">{t("bootstrapGpuSupported")}</p>
              ) : (
                <p className="status-blocked">{t("bootstrapGpuUnsupported")}</p>
              )}
            </div>
            <div className="bootstrap-section">
              <h3>{t("bootstrapDownload")}</h3>
              {check.needsRuntime && <p>{t("bootstrapNeedRuntime")}</p>}
              {check.needsModels && <p>{t("bootstrapNeedModels")}</p>}
              <button className="btn btn-primary" onClick={handleStartDownload} disabled={!check.spaceOk}>
                {t("bootstrapStartDownload")}
              </button>
            </div>
          </div>
        </div>
      );
    }
    case "downloading":
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <p className="bootstrap-status">{phase.progress.status}</p>
            <p className="bootstrap-hint">{t("bootstrapDownloadHint")}</p>
          </div>
        </div>
      );
    case "installing":
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <p className="bootstrap-status">{phase.message}</p>
          </div>
        </div>
      );
    case "complete":
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <p className="bootstrap-status status-ready">{t("bootstrapComplete")}</p>
            <button className="btn btn-primary" onClick={() => onComplete(phase.state)}>
              {t("bootstrapContinue")}
            </button>
          </div>
        </div>
      );
    case "error":
      return (
        <div className="bootstrap-panel">
          <div className="bootstrap-card">
            <h2>{t("bootstrapTitle")}</h2>
            <div className="banner error">
              {t("errorBanner")}: {phase.message}
            </div>
            <button className="btn" onClick={() => setPhase({ kind: "checking" })}>
              {t("bootstrapRetry")}
            </button>
          </div>
        </div>
      );
  }
}
