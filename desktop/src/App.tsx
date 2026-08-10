import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [shellStatus, setShellStatus] = useState<string>("checking...");

  useEffect(() => {
    let cancelled = false;
    invoke<string>("ping")
      .then((result) => {
        if (!cancelled) setShellStatus(`rust shell: ${result}`);
      })
      .catch((error) => {
        if (!cancelled) setShellStatus(`rust shell: error (${error})`);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="container">
      <h1>DubFlow</h1>
      <p className="subtitle">เครื่องมือสร้างเสียงพากย์ไทยจาก SRT</p>
      <p data-testid="shell-status">{shellStatus}</p>
    </main>
  );
}

export default App;
