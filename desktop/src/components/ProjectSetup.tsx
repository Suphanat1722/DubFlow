import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n";
import type { Project } from "../lib/types";

export function ProjectSetup({
  onProject,
  onError,
}: {
  onProject: (project: Project) => void;
  onError: (message: string) => void;
}) {
  const [name, setName] = useState("");
  const [videoPath, setVideoPath] = useState("");
  const [srtPath, setSrtPath] = useState("");
  const [parentDir, setParentDir] = useState("");
  const [busy, setBusy] = useState(false);

  const pickVideo = async () => {
    const result = await open({
      title: t("chooseVideo"),
      multiple: false,
      filters: [{ name: "MP4 Video", extensions: ["mp4"] }],
    });
    if (typeof result === "string") setVideoPath(result);
  };

  const pickSrt = async () => {
    const result = await open({
      title: t("chooseSrt"),
      multiple: false,
      filters: [{ name: "SRT Subtitles", extensions: ["srt"] }],
    });
    if (typeof result === "string") setSrtPath(result);
  };

  const pickFolder = async () => {
    const result = await open({
      title: t("chooseFolder"),
      directory: true,
      multiple: false,
    });
    if (typeof result === "string") setParentDir(result);
  };

  const canCreate =
    name.trim().length > 0 && videoPath && srtPath && parentDir && !busy;

  const createProject = async () => {
    if (!canCreate) return;
    setBusy(true);
    try {
      const project = await invoke<Project>("project_create", {
        parentDir,
        name: name.trim(),
        videoPath,
        srtPath,
      });
      onProject(project);
    } catch (err) {
      onError(typeof err === "string" ? err : String(err));
    } finally {
      setBusy(false);
    }
  };

  const openExisting = async () => {
    const result = await open({
      title: t("openProjectDir"),
      directory: true,
      multiple: false,
    });
    if (typeof result !== "string") return;
    setBusy(true);
    try {
      const project = await invoke<Project>("project_open", {
        projectDir: result,
      });
      onProject(project);
    } catch (err) {
      onError(typeof err === "string" ? err : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setup-screen">
      <div className="setup-card">
        <h1 className="setup-title">{t("setupTitle")}</h1>
        <p className="setup-subtitle">{t("readyToStart")}</p>

        <label className="field">
          <span className="field-label">{t("projectName")}</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("projectNamePlaceholder")}
            data-testid="project-name"
          />
        </label>

        <label className="field">
          <span className="field-label">{t("videoFile")}</span>
          <div className="file-row">
            <input
              readOnly
              value={videoPath}
              placeholder={t("videoNotSelected")}
              data-testid="video-path"
            />
            <button className="btn" onClick={pickVideo}>
              {t("chooseVideo")}
            </button>
          </div>
        </label>

        <label className="field">
          <span className="field-label">{t("srtFile")}</span>
          <div className="file-row">
            <input
              readOnly
              value={srtPath}
              placeholder={t("srtNotSelected")}
              data-testid="srt-path"
            />
            <button className="btn" onClick={pickSrt}>
              {t("chooseSrt")}
            </button>
          </div>
        </label>

        <label className="field">
          <span className="field-label">{t("projectFolder")}</span>
          <div className="file-row">
            <input
              readOnly
              value={parentDir}
              placeholder={t("folderNotSelected")}
              data-testid="parent-dir"
            />
            <button className="btn" onClick={pickFolder}>
              {t("chooseFolder")}
            </button>
          </div>
        </label>

        <div className="setup-actions">
          <button
            className="btn btn-primary"
            disabled={!canCreate}
            onClick={createProject}
            data-testid="create-project"
          >
            {busy ? t("creating") : t("createProject")}
          </button>
          <button className="btn" onClick={openExisting} disabled={busy}>
            {t("openProject")}
          </button>
        </div>
      </div>
    </div>
  );
}
