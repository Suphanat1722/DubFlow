//! Python sidecar process management and JSON-RPC client.
//!
//! The Rust shell spawns `python -m dubflow_worker.worker` with stdin/stdout
//! pipes and speaks the versioned line-delimited JSON-RPC protocol from
//! `specs/ipc.md`. A reader thread decodes every response and forwards it to
//! the shared queue; `call()` matches responses by request id.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use super::media::MediaError;

const PROTOCOL_VERSION: u32 = 1;
/// Longest wall-clock time a single synthesis may take before we treat the
/// worker as hung. Phase 1 measured ~50s for a 9.5s cue; 30 minutes is a
/// generous ceiling that still fails loudly instead of hanging forever.
const RPC_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

/// Structured worker error with a stable `kind` the shell can map to a
/// user-facing message (see `WorkerError::to_user_message`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub kind: String,
}

impl WorkerError {
    /// Map a worker error to a localized user-facing message.
    ///
    /// The mapping lives here so the UI never has to parse raw error text.
    pub fn to_user_message(&self) -> String {
        match self.kind.as_str() {
            "missing-file" => {
                "ไม่พบไฟล์เสียงต้นทาง โปรดตรวจสอบ path ของ reference".to_string()
            }
            "no-audio" => {
                "การสังเคราะห์ไม่สร้างเสียงออกมา โปรดลอง Regenerate อีกครั้ง".to_string()
            }
            "not-initialized" => {
                "ยังไม่ได้เริ่มต้นโมเดลเสียง โปรดตรวจสอบสถานะ runtime".to_string()
            }
            "out-of-memory" => {
                "หน่วยความจำ GPU ไม่พอ โปรดปิดแอปอื่นที่ใช้ GPU แล้วลองใหม่".to_string()
            }
            "cuda-error" => {
                "เกิดข้อผิดพลาด CUDA โปรดตรวจสอบไดรเวอร์และลองใหม่".to_string()
            }
            "timeout" => "งานใช้เวลานานเกินกำหนดและถูกยกเลิก โปรดลองอีกครั้ง".to_string(),
            _ => {
                if !self.message.trim().is_empty() {
                    format!("เกิดข้อผิดพลาด: {}", self.message)
                } else {
                    "เกิดข้อผิดพลาดที่ไม่รู้จัก".to_string()
                }
            }
        }
    }
}

/// Decoded JSON-RPC response.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcResponse {
    pub id: u64,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<WorkerError>,
}

/// A live Python worker process.
pub struct WorkerClient {
    process: Child,
    stdin: ChildStdin,
    responses: Receiver<RpcResponse>,
    next_id: u64,
    stdout_thread: Option<thread::JoinHandle<()>>,
}

impl WorkerClient {
    /// Spawn the Python worker sidecar.
    ///
    /// `python` defaults to `python` on PATH; callers that bundle a runtime
    /// pass the resolved interpreter path.
    pub fn spawn(python: Option<&Path>, cwd: Option<&Path>) -> Result<Self, WorkerError> {
        let mut cmd = Command::new(
            python
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("python")),
        );
        cmd.arg("-m")
            .arg("dubflow_worker.worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut process = cmd.spawn().map_err(|e| WorkerError {
            code: -1,
            message: format!("cannot spawn python worker: {e}"),
            kind: "spawn".to_string(),
        })?;
        let stdin = process.stdin.take().ok_or_else(|| WorkerError {
            code: -1,
            message: "worker stdin unavailable".to_string(),
            kind: "spawn".to_string(),
        })?;
        let stdout = process.stdout.take().ok_or_else(|| WorkerError {
            code: -1,
            message: "worker stdout unavailable".to_string(),
            kind: "spawn".to_string(),
        })?;

        let (tx, rx) = mpsc::channel();
        let stdout_thread = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                match serde_json::from_str::<RpcResponse>(&line) {
                    Ok(resp) => {
                        if tx.send(resp).is_err() {
                            break;
                        }
                    }
                    Err(_) => continue,
                }
            }
        });

        Ok(Self {
            process,
            stdin,
            responses: rx,
            next_id: 1,
            stdout_thread: Some(stdout_thread),
        })
    }

    /// Send a JSON-RPC request and wait for the matching response.
    pub fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WorkerError> {
        let id = self.next_id;
        self.next_id += 1;
        let request = serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION,
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&request).map_err(|e| WorkerError {
            code: -32600,
            message: e.to_string(),
            kind: "serialize".to_string(),
        })?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| WorkerError {
                code: -1,
                message: format!("cannot write to worker: {e}"),
                kind: "pipe".to_string(),
            })?;
        self.stdin.flush().map_err(|e| WorkerError {
            code: -1,
            message: format!("cannot flush worker stdin: {e}"),
            kind: "pipe".to_string(),
        })?;

        // Synthesis can take minutes; give it a long deadline. Everything
        // else is fast and gets a shorter timeout.
        let deadline = if method == "tts.synthesize" {
            RPC_TIMEOUT
        } else {
            SYNC_TIMEOUT
        };
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            match self.responses.recv_timeout(Duration::from_millis(250)) {
                Ok(resp) if resp.id == id => {
                    if let Some(err) = resp.error {
                        return Err(err);
                    }
                    return Ok(resp.result);
                }
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(WorkerError {
                        code: -1,
                        message: "worker exited before responding".to_string(),
                        kind: "disconnected".to_string(),
                    });
                }
            }
        }
        Err(WorkerError {
            code: -1,
            message: format!("worker did not respond to {method} in time"),
            kind: "timeout".to_string(),
        })
    }

    /// Close the worker process and wait for its stdout thread.
    pub fn close(&mut self) {
        let _ = self.stdin.flush();
        let _ = self.process.kill();
        let _ = self.process.wait();
        if let Some(handle) = self.stdout_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        self.close();
    }
}

impl From<MediaError> for WorkerError {
    fn from(e: MediaError) -> Self {
        WorkerError {
            code: -2,
            message: e.to_string(),
            kind: "media".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_error_maps_to_thai_message() {
        let err = WorkerError {
            code: -32601,
            message: "synthesis produced no audio".to_string(),
            kind: "no-audio".to_string(),
        };
        assert!(err.to_user_message().contains("Regenerate"));

        let generic = WorkerError {
            code: -32601,
            message: "boom".to_string(),
            kind: "".to_string(),
        };
        assert!(generic.to_user_message().contains("boom"));

        let timeout = WorkerError {
            code: -1,
            message: "timeout".to_string(),
            kind: "timeout".to_string(),
        };
        assert!(timeout.to_user_message().contains("ยกเลิก"));
    }

    /// Spawn the real Python worker (from python-worker/) and ping it.
    /// Skips when python is not on PATH (CI has it via setup-python).
    #[test]
    fn spawns_worker_and_pings() {
        // The worker module is in the repo root's python-worker/ directory.
        let py_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("python-worker"));
        let mut worker = match WorkerClient::spawn(None, py_dir.as_deref()) {
            Ok(w) => w,
            Err(_) => return, // python not available; skip
        };
        let result = worker
            .call("system.ping", serde_json::json!({}))
            .expect("ping should succeed");
        assert_eq!(result["pong"], true);
        worker.close();
    }
}
