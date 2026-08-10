//! GPU job queue, take operations, stretch cache, and solver integration.
//!
//! ## Job queue
//!
//! Jobs are sequential (one at a time). `cancel_after_current` clears the
//! pending queue so the next job after the currently-running one is a no-op.
//!
//! ## Take operations
//!
//! - Generate: call worker synthesize, create Take, add to cue, select it.
//! - Regenerate: same but with an explicit seed.
//! - Select: just set `selected_take_id`.
//! - Delete: remove take reference and its audio file.
//!
//! ## Stretch cache
//!
//! When a take is selected and the solver assigns speed > 1.0, the shell
//! calls `get_or_create_stretched` to produce a stretched + normalized
//! version. The cache is keyed by `(take_id, speed)` and the raw take file
//! is never modified.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use serde::Serialize;

use crate::domain::media;
use crate::domain::project::Project;
use crate::domain::take::{Cue, Take};
use crate::domain::timeline;
use crate::domain::worker::{WorkerClient, WorkerError};

// ---------------------------------------------------------------------------
// Job queue
// ---------------------------------------------------------------------------

/// A single queued generation job.
#[derive(Debug, Clone)]
pub struct Job {
    pub cue_id: String,
    pub action: JobAction,
}

/// What to do for a cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobAction {
    /// Generate a new take with a random seed.
    Generate,
    /// Regenerate with a specific seed (replaces the former selected take).
    Regenerate { seed: u64 },
}

/// Events emitted by the job queue so the UI can react.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JobEvent {
    /// A job was queued.
    Queued { cue_id: String },
    /// A job started running.
    Started { cue_id: String },
    /// A job completed successfully.
    Completed {
        cue_id: String,
        take_id: String,
        duration_ms: u64,
    },
    /// A job failed.
    Failed { cue_id: String, error: WorkerError },
    /// The queue was cleared (cancel).
    Cancelled { cue_id: String },
}

/// Sequential job queue for GPU-bound TTS work.
pub struct JobQueue {
    pending: Vec<Job>,
    current: Option<Job>,
    cancelled: bool,
    events: Sender<JobEvent>,
    events_rx: Receiver<JobEvent>,
}

impl JobQueue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            pending: Vec::new(),
            current: None,
            cancelled: false,
            events: tx,
            events_rx: rx,
        }
    }

    /// Drain any pending events for the UI.
    pub fn drain_events(&mut self) -> Vec<JobEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.events_rx.try_recv() {
            out.push(ev);
        }
        out
    }

    /// Enqueue a job.
    pub fn enqueue(&mut self, job: Job) {
        self.cancelled = false;
        let _ = self.events.send(JobEvent::Queued {
            cue_id: job.cue_id.clone(),
        });
        self.pending.push(job);
    }

    /// Enqueue many jobs (Generate All).
    pub fn enqueue_all(&mut self, jobs: Vec<Job>) {
        self.cancelled = false;
        for job in jobs {
            let _ = self.events.send(JobEvent::Queued {
                cue_id: job.cue_id.clone(),
            });
            self.pending.push(job);
        }
    }

    /// Cancel after the current job. Clears the pending queue.
    pub fn cancel_after_current(&mut self) {
        self.cancelled = true;
        for job in self.pending.drain(..) {
            let _ = self.events.send(JobEvent::Cancelled {
                cue_id: job.cue_id,
            });
        }
    }

    /// Returns `true` if there is a current or pending job.
    pub fn is_busy(&self) -> bool {
        self.current.is_some() || !self.pending.is_empty()
    }

    /// Returns `true` when there is a next job to run (not cancelled, queue
    /// non-empty).
    pub fn has_next(&self) -> bool {
        !self.cancelled && !self.pending.is_empty()
    }

    /// Pop the next job and mark it as the current one without emitting
    /// events. The caller is responsible for emitting lifecycle events (the
    /// UI-facing run loop pushes them to its own event log).
    pub fn pop_job(&mut self) -> Option<Job> {
        if self.cancelled || self.pending.is_empty() {
            return None;
        }
        let job = self.pending.remove(0);
        self.current = Some(job.clone());
        Some(job)
    }

    /// Clear the current-job marker after a job finished.
    pub fn finish_job(&mut self) {
        self.current = None;
    }

    /// Push an event into the queue's event channel so the UI can pick it up
    /// via `drain_events`. Used by external run loops that don't hold the
    /// queue lock throughout the entire execution.
    pub fn push_event(&mut self, event: JobEvent) {
        let _ = self.events.send(event);
    }

    /// Update all cue statuses by running the solver.
    pub fn update_solver(project: &mut Project) {
        let inputs: Vec<_> = project
            .cues
            .iter()
            .map(|c| {
                timeline::SolverInput::new(
                    c.id.clone(),
                    c.srt_start_ms,
                    c.srt_end_ms,
                    c.selected_duration_ms(),
                )
            })
            .collect();
        let result = timeline::solve(&inputs, None);
        for solved in &result.cues {
            if let Some(cue) = project.cues.iter_mut().find(|c| c.id == solved.cue_id) {
                cue.status = solved.status;
            }
        }
    }

    /// Run the next job from the queue using the given worker and project.
    ///
    /// This is a blocking call — call it from a dedicated worker thread or
    /// spawn a background task. Returns the event for the completed/failed
    /// job, or `None` if the queue is empty or was cancelled.
    pub fn run_next(
        &mut self,
        worker: &mut WorkerClient,
        project: &mut Project,
        reference_audio_path: &str,
        reference_transcript: &str,
        settings: &serde_json::Value,
    ) -> Option<JobEvent> {
        let job = self.pop_job()?;
        let _ = self.events.send(JobEvent::Started {
            cue_id: job.cue_id.clone(),
        });
        let ev = execute_job(
            &job,
            worker,
            project,
            reference_audio_path,
            reference_transcript,
            settings,
        );
        self.current = None;
        let _ = self.events.send(ev.clone());
        Some(ev)
    }

    /// Run all queued jobs sequentially.
    pub fn run_all(
        &mut self,
        worker: &mut WorkerClient,
        project: &mut Project,
        reference_audio_path: &str,
        reference_transcript: &str,
        settings: &serde_json::Value,
    ) -> Vec<JobEvent> {
        let mut events = Vec::new();
        while !self.pending.is_empty() && !self.cancelled {
            if let Some(ev) = self.run_next(
                worker,
                project,
                reference_audio_path,
                reference_transcript,
                settings,
            ) {
                events.push(ev);
            }
        }
        events
    }
}

/// Execute a single job against the worker and project without touching the
/// queue. Returns the terminal event (Completed or Failed); the caller
/// decides how to surface it.
pub fn execute_job(
    job: &Job,
    worker: &mut WorkerClient,
    project: &mut Project,
    reference_audio_path: &str,
    reference_transcript: &str,
    settings: &serde_json::Value,
) -> JobEvent {
    // Find the cue.
    let Some(cue_idx) = project.cues.iter().position(|c| c.id == job.cue_id) else {
        return JobEvent::Failed {
            cue_id: job.cue_id.clone(),
            error: WorkerError {
                code: -1,
                message: "cue not found".to_string(),
                kind: "cue-not-found".to_string(),
            },
        };
    };

    let cue = &project.cues[cue_idx];
    let text = cue.text.clone();
    let seed = match job.action {
        JobAction::Generate => {
            // Use a random seed.
            use rand::Rng;
            rand::thread_rng().gen_range(1..1_000_000_000)
        }
        JobAction::Regenerate { seed } => seed,
    };

    // Preprocess reference if needed (the worker caches internally).
    let preprocess_result = worker.call(
        "tts.preprocess_reference",
        serde_json::json!({
            "audioPath": reference_audio_path,
            "transcript": reference_transcript,
        }),
    );
    let reference = match preprocess_result {
        Ok(v) => v,
        Err(e) => {
            return JobEvent::Failed {
                cue_id: job.cue_id.clone(),
                error: e,
            };
        }
    };

    // Synthesize.
    let synthesize_result = worker.call(
        "tts.synthesize",
        serde_json::json!({
            "reference": {
                "audioPath": reference["audioPath"],
                "transcript": reference["transcript"],
                "durationMs": reference["durationMs"],
                "sampleRate": reference["sampleRate"],
                "sha256": reference["sha256"],
            },
            "text": text,
            "seed": seed,
            "settings": settings,
        }),
    );
    let result = match synthesize_result {
        Ok(v) => v,
        Err(e) => {
            return JobEvent::Failed {
                cue_id: job.cue_id.clone(),
                error: e,
            };
        }
    };

    let audio_path = result["audioPath"].as_str().unwrap_or("").to_string();
    let duration_ms = result["durationMs"].as_u64().unwrap_or(0);
    let settings_hash = result["settingsHash"].as_str().unwrap_or("").to_string();

    // Unique take id per cue: seed + per-cue counter. The worker writes
    // `take-{seed}.wav`; we rename it into the project `takes/` folder so
    // two takes with the same seed never overwrite each other and the
    // stored path is project-relative (survives project relocation).
    let counter = project.cues[cue_idx].takes.len() + 1;
    let take_id = format!("take-{seed}-{counter:02}");
    let project_dir = project.project_dir.clone();
    let final_audio = project_dir
        .join("takes")
        .join(format!("{take_id}.wav"));
    if let Some(parent) = final_audio.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let worker_out = Path::new(&audio_path);
    if worker_out != final_audio {
        let _ = std::fs::rename(worker_out, &final_audio);
    }
    let relative_audio = format!("takes/{}", final_audio.file_name().unwrap().to_string_lossy());

    // Create the Take and add it to the cue.
    let take = Take {
        take_id: take_id.clone(),
        cue_id: job.cue_id.clone(),
        provider: "jaitts-f5tts".to_string(),
        provider_version: "1.1.22".to_string(),
        seed,
        duration_ms,
        settings_hash,
        audio_path: relative_audio,
    };
    let cue = &mut project.cues[cue_idx];
    cue.takes.push(take);
    cue.selected_take_id = Some(take_id.clone());
    cue.dirty = true;

    // Run the solver to update cue status.
    JobQueue::update_solver(project);

    JobEvent::Completed {
        cue_id: job.cue_id.clone(),
        take_id,
        duration_ms,
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Take operations (non-queue)
// ---------------------------------------------------------------------------

/// Select a take for a cue. Does not modify any audio file.
pub fn select_take(project: &mut Project, cue_id: &str, take_id: &str) -> Result<(), String> {
    let cue = project
        .cues
        .iter_mut()
        .find(|c| c.id == cue_id)
        .ok_or_else(|| format!("cue {cue_id} not found"))?;
    if !cue.takes.iter().any(|t| t.take_id == take_id) {
        return Err(format!("take {take_id} not found in cue {cue_id}"));
    }
    cue.selected_take_id = Some(take_id.to_string());
    cue.dirty = true;
    Ok(())
}

/// Delete a take (remove reference and optionally the audio file).
pub fn delete_take(
    project: &mut Project,
    cue_id: &str,
    take_id: &str,
    project_dir: &Path,
) -> Result<(), String> {
    let cue = project
        .cues
        .iter_mut()
        .find(|c| c.id == cue_id)
        .ok_or_else(|| format!("cue {cue_id} not found"))?;
    let Some(idx) = cue.takes.iter().position(|t| t.take_id == take_id) else {
        return Err(format!("take {take_id} not found"));
    };
    let take = cue.takes.remove(idx);
    // Remove the audio file if it exists inside the project.
    let audio_path = Path::new(&take.audio_path);
    if audio_path.is_absolute() {
        let _ = std::fs::remove_file(audio_path);
    } else {
        let full = project_dir.join(&take.audio_path);
        let _ = std::fs::remove_file(full);
    }
    // If the deleted take was selected, unselect.
    if cue.selected_take_id.as_deref() == Some(take_id) {
        cue.selected_take_id = None;
    }
    cue.dirty = true;
    Ok(())
}

/// Get the audio path for a take, returning the raw take path.
pub fn take_audio_path<'a>(cue: &'a Cue, take_id: &str) -> Option<&'a str> {
    cue.takes
        .iter()
        .find(|t| t.take_id == take_id)
        .map(|t| t.audio_path.as_str())
}

// ---------------------------------------------------------------------------
// Stretch cache
// ---------------------------------------------------------------------------

/// Compute the cache path for a stretched take.
///
/// Format: `cache/stretch-{take_id}-{speed_percent}.wav`
/// e.g. `cache/stretch-take-0001-125.wav` for 1.25x
pub fn stretch_cache_path(project_dir: &Path, take_id: &str, speed: f64) -> PathBuf {
    let pct = (speed * 100.0).round() as u32;
    project_dir
        .join("cache")
        .join(format!("stretch-{take_id}-{pct}.wav"))
}

/// Get or create a stretched + normalized version of a take.
///
/// If the cached file exists, returns its path. Otherwise stretches the
/// raw take, normalizes to -18 LUFS / -1.5 dBTP, writes to cache, and
/// returns the cache path.
pub fn get_or_create_stretched(
    project_dir: &Path,
    take: &Take,
    speed: f64,
) -> Result<PathBuf, WorkerError> {
    let cache_path = stretch_cache_path(project_dir, &take.take_id, speed);
    if cache_path.is_file() {
        return Ok(cache_path);
    }
    let raw = Path::new(&take.audio_path);
    let raw = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        project_dir.join(raw)
    };
    if !raw.is_file() {
        return Err(WorkerError {
            code: -2,
            message: format!("raw take audio not found: {}", raw.display()),
            kind: "missing-file".to_string(),
        });
    }
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Stretch, then normalize. Raw take is never modified.
    let tmp = project_dir.join("cache").join(format!("stretch-tmp-{}.wav", take.take_id));
    media::pitch_preserving_stretch(&raw, &tmp, speed).map_err(|e| WorkerError {
        code: -2,
        message: format!("stretch failed: {e}"),
        kind: "media".to_string(),
    })?;
    let norm_tmp = project_dir.join("cache").join(format!("norm-tmp-{}.wav", take.take_id));
    media::normalize_loudness(&tmp, &norm_tmp, -18.0, -1.5).map_err(|e| WorkerError {
        code: -2,
        message: format!("normalization failed: {e}"),
        kind: "media".to_string(),
    })?;
    let _ = std::fs::rename(&norm_tmp, &cache_path);
    let _ = std::fs::remove_file(&tmp);
    Ok(cache_path)
}

/// Clear the stretch cache for a take.
pub fn clear_stretch_cache(project_dir: &Path, take_id: &str) {
    let cache_dir = project_dir.join("cache");
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&format!("stretch-{take_id}-"))
                || name.starts_with(&format!("stretch-tmp-{take_id}"))
                || name.starts_with(&format!("norm-tmp-{take_id}"))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::take::Cue;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_CACHE_DIR: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir() -> PathBuf {
        let n = NEXT_CACHE_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dubflow_job_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cache")).unwrap();
        std::fs::create_dir_all(dir.join("takes")).unwrap();
        dir
    }

    fn write_tone_wav(path: &Path, seconds: f64, freq: u32) {
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency={freq}:duration={seconds}"),
                "-ar",
                "48000",
                "-ac",
                "1",
            ])
            .arg(path)
            .status()
            .expect("ffmpeg should be available");
        assert!(status.success(), "ffmpeg failed to write test tone");
    }

    #[test]
    fn job_queue_round_trip() {
        let mut queue = JobQueue::new();
        assert!(!queue.is_busy());

        queue.enqueue(Job {
            cue_id: "cue-001".to_string(),
            action: JobAction::Generate,
        });
        assert!(queue.is_busy());

        let events = queue.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], JobEvent::Queued { .. }));

        queue.cancel_after_current();
        assert!(!queue.is_busy());
    }

    #[test]
    fn select_take_works() {
        let mut cue = Cue::new("cue-001".to_string(), 1, "hello".to_string(), 0, 1000);
        cue.takes.push(Take {
            take_id: "take-001".to_string(),
            cue_id: "cue-001".to_string(),
            provider: "test".to_string(),
            provider_version: "1.0".to_string(),
            seed: 42,
            duration_ms: 500,
            settings_hash: "abc".to_string(),
            audio_path: "takes/take-001.wav".to_string(),
        });
        let mut project = Project::new(
            "test".to_string(),
            PathBuf::from("."),
            "video.mp4".to_string(),
            "subs.srt".to_string(),
        );
        project.cues.push(cue);

        assert!(select_take(&mut project, "cue-001", "take-001").is_ok());
        assert_eq!(project.cues[0].selected_take_id, Some("take-001".into()));

        assert!(select_take(&mut project, "cue-001", "nonexistent").is_err());
    }

    #[test]
    fn stretch_cache_path_format() {
        let dir = PathBuf::from("/project");
        let path = stretch_cache_path(&dir, "take-0042", 1.25);
        let expected = Path::new("/project")
            .join("cache")
            .join("stretch-take-0042-125.wav");
        assert_eq!(path, expected);
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "stretch-take-0042-125.wav"
        );
    }

    #[test]
    fn delete_take_unselects_if_selected() {
        let mut cue = Cue::new("cue-001".to_string(), 1, "hello".to_string(), 0, 1000);
        cue.takes.push(Take {
            take_id: "take-001".to_string(),
            cue_id: "cue-001".to_string(),
            provider: "test".to_string(),
            provider_version: "1.0".to_string(),
            seed: 42,
            duration_ms: 500,
            settings_hash: "abc".to_string(),
            audio_path: "takes/take-001.wav".to_string(),
        });
        let mut project = Project::new(
            "test".to_string(),
            PathBuf::from("."),
            "video.mp4".to_string(),
            "subs.srt".to_string(),
        );
        project.cues.push(cue);
        select_take(&mut project, "cue-001", "take-001").unwrap();

        assert!(delete_take(&mut project, "cue-001", "take-001", Path::new(".")).is_ok());
        assert!(project.cues[0].selected_take_id.is_none());
        assert!(project.cues[0].takes.is_empty());
    }

    /// End-to-end stretch + normalize through FFmpeg. Verifies the raw take
    /// is never modified and the cache is reused on the second call.
    #[test]
    fn stretch_cache_is_non_destructive_and_reused() {
        let dir = tmp_dir();
        let raw_path = dir.join("takes").join("take-0042.wav");
        write_tone_wav(&raw_path, 2.0, 440);

        let take = Take {
            take_id: "take-0042".to_string(),
            cue_id: "cue-001".to_string(),
            provider: "jaitts-f5tts".to_string(),
            provider_version: "1.1.22".to_string(),
            seed: 42,
            duration_ms: 2000,
            settings_hash: "abc".to_string(),
            audio_path: "takes/take-0042.wav".to_string(),
        };

        let raw_before = std::fs::read(&raw_path).unwrap();
        let cached = get_or_create_stretched(&dir, &take, 1.25).expect("stretch should succeed");
        assert!(cached.is_file());
        assert_eq!(cached.file_name().unwrap().to_string_lossy(), "stretch-take-0042-125.wav");
        let raw_after = std::fs::read(&raw_path).unwrap();
        assert_eq!(raw_before, raw_after, "raw take must not be modified");

        // Second call should reuse the cache without re-stretching.
        let cached2 = get_or_create_stretched(&dir, &take, 1.25).expect("reuse should succeed");
        assert_eq!(cached, cached2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
