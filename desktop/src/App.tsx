import { useState } from "react";
import "./App.css";
import { t } from "./lib/i18n";
import { isTauri } from "./lib/assetUrl";
import { ProjectSetup } from "./components/ProjectSetup";
import { Editor } from "./components/Editor";
import { BootstrapPanel } from "./components/BootstrapPanel";
import type { Project } from "./lib/types";

function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [bootstrapReady, setBootstrapReady] = useState(false);

  if (!isTauri()) {
    return (
      <main className="container">
        <h1>{t("appTitle")}</h1>
        <p className="subtitle">{t("appSubtitle")}</p>
        <p className="not-tauri">This app requires the Tauri desktop shell.</p>
      </main>
    );
  }

  if (!project) {
    return (
      <main className="container">
        <h1>{t("appTitle")}</h1>
        <p className="subtitle">{t("appSubtitle")}</p>
        {error && (
          <div className="banner error" onClick={() => setError(null)}>
            {t("errorBanner")}: {error}
          </div>
        )}
        {bootstrapReady ? (
          <ProjectSetup onProject={setProject} onError={setError} />
        ) : (
          <BootstrapPanel
            onComplete={() => setBootstrapReady(true)}
          />
        )}
      </main>
    );
  }

  return (
    <div className="app">
      <Editor project={project} onClose={() => setProject(null)} />
    </div>
  );
}

export default App;
