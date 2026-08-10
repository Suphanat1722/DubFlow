pub mod domain;

use std::path::PathBuf;
use std::sync::Mutex;

use domain::job::{self, Job, JobAction, JobQueue};
use domain::export::{self, ExportMode, ExportRequest};
use domain::project::{Project, ProjectError};
use domain::reference;
use domain::srt::parse_srt;
use domain::take::Cue;
use domain::timeline;
use domain::worker::WorkerClient;
use domain::bootstrap::{self, BootstrapCheckResult, BootstrapState, DownloadProgress, GpuInfo};

/// App state shared between Tauri commands.
pub struct AppState {
    /// The currently open project (if any).
    pub project: Mutex<Option<Project>>,
    /// The Python worker sidecar.
    pub worker: Mutex<Option<WorkerClient>>,
    /// Sequential generation job queue.
    pub jobs: Mutex<JobQueue>,
    /// Bootstrap download progress (shared for UI polling).
    pub download_progress: Mutex<DownloadProgress>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: Mutex::new(None),
            worker: Mutex::new(None),
            jobs: Mutex::new(JobQueue::new()),
            download_progress: Mutex::new(DownloadProgress {
                total_bytes: 0,
                downloaded_bytes: 0,
                current_file: String::new(),
                status: String::new(),
            }),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn worker_error_to_command(err: domain::worker::WorkerError) -> String {
    err.to_user_message()
}

fn project_error_to_command(err: ProjectError) -> String {
    match err {
        ProjectError::Worker { message } => format!("worker: {message}"),
        ProjectError::InvalidReference { message } => format!("invalid reference: {message}"),
        ProjectError::MissingReferenceAudio { path } => {
            format!("missing reference audio: {path}")
        }
        other => other.to_string(),
    }
}

fn export_mode_from_str(mode: &str) -> Result<ExportMode, String> {
    match mode {
        "replace" => Ok(ExportMode::Replace),
        "mix" => Ok(ExportMode::Mix),
        "voiceTrack" | "voice-track" => Ok(ExportMode::VoiceTrack),
        _ => Err(format!("unknown export mode: {mode}")),
    }
}

/// Run a bootstrap health check (GPU detection, license state, disk space).
#[tauri::command]
fn bootstrap_check() -> BootstrapCheckResult {
    bootstrap::run_bootstrap_check()
}

/// Accept a model license and persist the decision.
#[tauri::command]
fn bootstrap_accept_license(model_id: String) -> Result<BootstrapState, String> {
    let state_path = bootstrap::default_state_path();
    bootstrap::accept_license(&state_path, &model_id).map_err(|e| e.to_string())
}

/// Poll the current download progress.
#[tauri::command]
fn bootstrap_download_progress(
    state: tauri::State<'_, AppState>,
) -> DownloadProgress {
    state.download_progress.lock().unwrap().clone()
}

/// Ensure bootstrap directories exist.
#[tauri::command]
fn bootstrap_ensure_dirs() -> Result<String, String> {
    let dir = bootstrap::ensure_dirs().map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

/// Detect GPU info.
#[tauri::command]
fn bootstrap_detect_gpu() -> GpuInfo {
    bootstrap::detect_gpu()
}

/// Verify a file's SHA-256 checksum.
#[tauri::command]
fn bootstrap_verify_checksum(path: String, expected_sha256: String) -> Result<bool, String> {
    bootstrap::verify_checksum(std::path::Path::new(&path), &expected_sha256)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// Get the current bootstrap state.
#[tauri::command]
fn bootstrap_state() -> BootstrapState {
    let state_path = bootstrap::default_state_path();
    bootstrap::load_state(&state_path)
}

/// Run the full bootstrap install pipeline (runtime + models) synchronously.
/// The UI polls `bootstrap_download_progress` while this runs.
#[tauri::command]
fn bootstrap_run_install(
    state: tauri::State<'_, AppState>,
) -> Result<BootstrapState, String> {
    let state_path = bootstrap::default_state_path();
    let manifest_path = bootstrap::manifest_path();
    let manifest = bootstrap::read_manifest(&manifest_path).map_err(|e| e.to_string())?;
    let progress = &state.download_progress;
    bootstrap::run_install(&state_path, &manifest, progress).map_err(|e| e.to_string())
}

/// Find the bundled or system ffmpeg, returning its path.
#[tauri::command]
fn bootstrap_find_ffmpeg() -> Result<String, String> {
    let bundled = bootstrap::data_dir().join("ffmpeg").join("ffmpeg.exe");
    if bundled.is_file() {
        return Ok(bundled.to_string_lossy().to_string());
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("ffmpeg.exe");
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }
    Err("ffmpeg not found on PATH or bundled".to_string())
}

/// Find the bundled or system ffprobe, returning its path.
#[tauri::command]
fn bootstrap_find_ffprobe() -> Result<String, String> {
    let bundled = bootstrap::data_dir().join("ffmpeg").join("ffprobe.exe");
    if bundled.is_file() {
        return Ok(bundled.to_string_lossy().to_string());
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("ffprobe.exe");
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }
    Err("ffprobe not found on PATH or bundled".to_string())
}

/// Simple ping command so the UI can verify the Rust shell is alive.
#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

/// Spawn the worker and configure its output directory.
#[tauri::command]
fn worker_spawn(
    state: tauri::State<'_, AppState>,
    python_path: Option<String>,
    worker_dir: Option<String>,
) -> Result<String, String> {
    let mut guard = state.worker.lock().unwrap();
    if guard.is_some() {
        return Ok("already-running".to_string());
    }
    let python = python_path.map(PathBuf::from);
    let dir = worker_dir.map(PathBuf::from);
    let worker = WorkerClient::spawn(python.as_deref(), dir.as_deref())
        .map_err(worker_error_to_command)?;
    *guard = Some(worker);
    Ok("spawned".to_string())
}

#[tauri::command]
fn worker_configure(
    state: tauri::State<'_, AppState>,
    output_dir: String,
) -> Result<serde_json::Value, String> {
    let mut guard = state.worker.lock().unwrap();
    let worker = guard
        .as_mut()
        .ok_or_else(|| "worker not running".to_string())?;
    worker
        .call(
            "worker.configure",
            serde_json::json!({ "outputDir": output_dir }),
        )
        .map_err(worker_error_to_command)
}

#[tauri::command]
fn worker_close(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.worker.lock().unwrap();
    if let Some(mut w) = guard.take() {
        w.close();
    }
    Ok(())
}

#[tauri::command]
fn worker_ping(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let mut guard = state.worker.lock().unwrap();
    let worker = guard
        .as_mut()
        .ok_or_else(|| "worker not running".to_string())?;
    worker
        .call("system.ping", serde_json::json!({}))
        .map_err(worker_error_to_command)
}

/// Initialize the TTS provider (loads model on GPU).
#[tauri::command]
fn tts_initialize(
    state: tauri::State<'_, AppState>,
    options: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut guard = state.worker.lock().unwrap();
    let worker = guard
        .as_mut()
        .ok_or_else(|| "worker not running".to_string())?;
    worker
        .call("tts.initialize", options)
        .map_err(worker_error_to_command)
}

/// Build a reference voice from the project video segment.
#[tauri::command]
fn reference_build_video_segment(
    state: tauri::State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
    transcript: String,
) -> Result<serde_json::Value, String> {
    let mut project_guard = state.project.lock().unwrap();
    let project = project_guard
        .as_mut()
        .ok_or_else(|| "no project open".to_string())?;
    let out_path = project.project_dir.join("reference").join("reference.wav");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create reference dir: {e}"))?;
    }
    let result = reference::build_video_segment_reference(
        project,
        reference::ReferenceSource::VideoSegment {
            start_ms,
            end_ms,
            transcript,
        },
        &out_path,
    )
    .map_err(project_error_to_command)?;
    project.reference = Some(result.reference.clone());
    project.dirty = true;
    Ok(serde_json::json!({
        "reference": result.reference,
        "durationMs": result.duration_ms,
    }))
}

/// Build a reference voice from an external audio file.
#[tauri::command]
fn reference_build_external(
    state: tauri::State<'_, AppState>,
    audio_path: String,
    transcript: String,
) -> Result<serde_json::Value, String> {
    let mut project_guard = state.project.lock().unwrap();
    let project = project_guard
        .as_mut()
        .ok_or_else(|| "no project open".to_string())?;
    let out_path = project.project_dir.join("reference").join("reference.wav");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create reference dir: {e}"))?;
    }
    let result = reference::build_external_reference(project, &audio_path, &transcript, &out_path)
        .map_err(project_error_to_command)?;
    project.reference = Some(result.reference.clone());
    project.dirty = true;
    Ok(serde_json::json!({
        "reference": result.reference,
        "durationMs": result.duration_ms,
    }))
}

/// Get the current project snapshot as JSON.
#[tauri::command]
fn project_get(state: tauri::State<'_, AppState>) -> Result<Option<serde_json::Value>, String> {
    let guard = state.project.lock().unwrap();
    Ok(guard.as_ref().map(|p| {
        let mut value = serde_json::to_value(p).unwrap_or_default();
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "projectDir".to_string(),
                serde_json::Value::String(p.project_dir.to_string_lossy().to_string()),
            );
        }
        value
    }))
}

/// Create a new project from a video and SRT file.
#[tauri::command]
fn project_create(
    state: tauri::State<'_, AppState>,
    parent_dir: String,
    name: String,
    video_path: String,
    srt_path: String,
) -> Result<serde_json::Value, String> {
    let project = domain::project::create_project(
        std::path::Path::new(&parent_dir),
        &name,
        &video_path,
        &srt_path,
    )
    .map_err(|e| e.to_string())?;

    // Parse the SRT to populate cues
    let srt_text = std::fs::read_to_string(&srt_path).map_err(|e| format!("cannot read SRT: {e}"))?;
    let parsed = parse_srt(&srt_text, None).map_err(|e| format!("SRT parse error: {}", e.message))?;
    let mut project = project;
    project.cues = parsed
        .cues
        .into_iter()
        .map(|sc| Cue::new(
            format!("cue-{:04}", sc.index),
            sc.index,
            sc.text,
            sc.start_ms,
            sc.end_ms,
        ))
        .collect();
    project.dirty = true;
    project.save().map_err(|e| e.to_string())?;

    let value = serde_json::to_value(&project).map_err(|e| e.to_string())?;
    let project_dir_str = project.project_dir.to_string_lossy().to_string();
    let mut guard = state.project.lock().unwrap();
    *guard = Some(project);
    let mut enriched = value;
    if let Some(obj) = enriched.as_object_mut() {
        obj.insert(
            "projectDir".to_string(),
            serde_json::Value::String(project_dir_str),
        );
    }
    Ok(enriched)
}

/// Save the current project.
#[tauri::command]
fn project_save(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.project.lock().unwrap();
    let project = guard.as_mut().ok_or_else(|| "no project open".to_string())?;
    project.save().map_err(|e| e.to_string())
}

/// Close the current project (returns it to saved state).
#[tauri::command]
fn project_close(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.project.lock().unwrap();
    if let Some(ref mut project) = *guard {
        if project.dirty {
            project.save().map_err(|e| e.to_string())?;
        }
    }
    *guard = None;
    Ok(())
}

/// Probe the video duration using ffprobe.
#[tauri::command]
fn probe_video_duration(video_path: String) -> Result<i64, String> {
    let path = std::path::Path::new(&video_path);
    domain::media::probe_duration(path).map_err(|e| e.to_string())
}

/// Update a cue's text in the current project.
#[tauri::command]
fn cue_update_text(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    text: String,
) -> Result<(), String> {
    let mut guard = state.project.lock().unwrap();
    let project = guard.as_mut().ok_or_else(|| "no project open".to_string())?;
    let cue = project
        .cues
        .iter_mut()
        .find(|c| c.id == cue_id)
        .ok_or_else(|| format!("cue {cue_id} not found"))?;
    cue.text = text;
    cue.dirty = true;
    project.dirty = true;
    Ok(())
}

/// Get the current export-blocked status.
#[tauri::command]
fn export_blocked(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    Ok(timeline::export_blocked_by_cue_status(&project.cues))
}

/// Validate whether the project can be exported in a given mode.
#[tauri::command]
fn export_validate(
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<serde_json::Value, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let mode = export_mode_from_str(&mode)?;
    let video_path = std::path::Path::new(&project.video.path);
    let video_has_audio = match domain::media::has_audio_stream(video_path) {
        Ok(v) => v,
        Err(e) => {
            // Missing/corrupt video blocks export; surface it as a reason so
            // the UI can show the blocker instead of a bare command error.
            return Ok(serde_json::json!({
                "exportBlocked": true,
                "reasons": [format!("video probe failed: {e}")],
            }));
        }
    };
    let video_duration = domain::media::probe_duration(std::path::Path::new(&project.video.path))
        .ok();
    let validation = export::validate_export(&project.cues, mode, video_has_audio, video_duration);
    serde_json::to_value(&validation).map_err(|e| e.to_string())
}

/// Export the project to `output_path` in the given mode.
///
/// This is a synchronous, blocking command: it renders the voice master and
/// runs FFmpeg. The UI invokes it from a button that disables itself while
/// it runs, so the command can stay simple for the MVP.
#[tauri::command]
fn export_run(
    state: tauri::State<'_, AppState>,
    mode: String,
    output_path: String,
    original_gain_db: Option<f64>,
) -> Result<serde_json::Value, String> {
    let mode = export_mode_from_str(&mode)?;
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let video_has_audio = domain::media::has_audio_stream(std::path::Path::new(&project.video.path))
        .map_err(|e| e.to_string())?;
    let request = ExportRequest {
        project_dir: project.project_dir.clone(),
        video_path: project.video.path.clone(),
        cues: project.cues.clone(),
        output_path: std::path::PathBuf::from(&output_path),
        mode,
        original_gain_db: original_gain_db.unwrap_or(-12.0),
    };
    let samples = export::run_export(&request, video_has_audio).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "samples": samples,
        "outputPath": output_path,
    }))
}

/// Get the current solver result for the project.
#[tauri::command]
fn solve_timeline(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
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
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

/// Get the project's take audio path for playback via asset protocol.
#[tauri::command]
fn take_audio_url(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    take_id: String,
) -> Result<String, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let cue = project
        .cues
        .iter()
        .find(|c| c.id == cue_id)
        .ok_or_else(|| format!("cue {cue_id} not found"))?;
    let take = cue
        .takes
        .iter()
        .find(|t| t.take_id == take_id)
        .ok_or_else(|| format!("take {take_id} not found"))?;
    Ok(take.audio_path.clone())
}

#[tauri::command]
fn project_open(
    state: tauri::State<'_, AppState>,
    project_dir: String,
) -> Result<serde_json::Value, String> {
    let mut guard = state.project.lock().unwrap();
    let project = Project::load(PathBuf::from(project_dir)).map_err(project_error_to_command)?;
    let value = serde_json::to_value(&project).map_err(|e| e.to_string())?;
    let project_dir_str = project.project_dir.to_string_lossy().to_string();
    *guard = Some(project);
    let mut enriched = value;
    if let Some(obj) = enriched.as_object_mut() {
        obj.insert(
            "projectDir".to_string(),
            serde_json::Value::String(project_dir_str),
        );
    }
    Ok(enriched)
}

/// Queue Generate All for every cue in the project.
#[tauri::command]
fn generate_all(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let cue_ids: Vec<String> = {
        let project = state.project.lock().unwrap();
        let Some(project) = project.as_ref() else {
            return Err("no project open".to_string());
        };
        project.cues.iter().map(|c| c.id.clone()).collect()
    };
    let jobs_vec: Vec<Job> = cue_ids
        .into_iter()
        .map(|cue_id| Job {
            cue_id,
            action: JobAction::Generate,
        })
        .collect();
    let count = jobs_vec.len();
    let mut jobs = state.jobs.lock().unwrap();
    jobs.enqueue_all(jobs_vec);
    Ok(count)
}

/// Queue Generate for a single cue.
#[tauri::command]
fn generate_one(
    state: tauri::State<'_, AppState>,
    cue_id: String,
) -> Result<(), String> {
    let mut jobs = state.jobs.lock().unwrap();
    jobs.enqueue(Job {
        cue_id,
        action: JobAction::Generate,
    });
    Ok(())
}

/// Queue Regenerate for a single cue with an explicit seed.
#[tauri::command]
fn regenerate_one(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    seed: u64,
) -> Result<(), String> {
    let mut jobs = state.jobs.lock().unwrap();
    jobs.enqueue(Job {
        cue_id,
        action: JobAction::Regenerate { seed },
    });
    Ok(())
}

/// Cancel after the current job.
#[tauri::command]
fn job_cancel_after_current(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.jobs.lock().unwrap().cancel_after_current();
    Ok(())
}

/// Drain queue events (poll from UI).
#[tauri::command]
fn job_drain_events(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut jobs = state.jobs.lock().unwrap();
    let events = jobs.drain_events();
    Ok(events
        .into_iter()
        .map(|ev| serde_json::to_value(ev).unwrap_or_default())
        .collect())
}

/// Select a take for a cue and update solver status.
#[tauri::command]
fn take_select(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    take_id: String,
) -> Result<(), String> {
    let mut project_guard = state.project.lock().unwrap();
    let project = project_guard
        .as_mut()
        .ok_or_else(|| "no project open".to_string())?;
    job::select_take(project, &cue_id, &take_id)?;
    JobQueue::update_solver(project);
    Ok(())
}

/// Delete a take.
#[tauri::command]
fn take_delete(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    take_id: String,
) -> Result<(), String> {
    let mut project_guard = state.project.lock().unwrap();
    let project = project_guard
        .as_mut()
        .ok_or_else(|| "no project open".to_string())?;
    let dir = project.project_dir.clone();
    job::delete_take(project, &cue_id, &take_id, &dir)?;
    JobQueue::update_solver(project);
    Ok(())
}

/// Run the queued jobs. This blocks until the queue drains or is cancelled.
///
/// The UI should call this from a background thread; Tauri commands already
/// run off the main thread.
#[tauri::command]
fn jobs_run(state: tauri::State<'_, AppState>) -> Result<Vec<serde_json::Value>, String> {
    let mut jobs = state.jobs.lock().unwrap();
    let mut project_guard = state.project.lock().unwrap();
    let project = project_guard
        .as_mut()
        .ok_or_else(|| "no project open".to_string())?;
    let mut worker_guard = state.worker.lock().unwrap();
    let worker = worker_guard
        .as_mut()
        .ok_or_else(|| "worker not running".to_string())?;

    // Determine reference audio + transcript.
    let reference = project.reference.as_ref().ok_or_else(|| {
        "no reference voice; build one before generating".to_string()
    })?;
    let reference_audio_path = reference.processed_audio_path.clone();
    let reference_transcript = reference.transcript.clone();
    let settings = serde_json::json!({
        "nfeStep": 32,
        "cfgStrength": 2.0,
        "swaySamplingCoef": -1.0,
        "speed": 1.0,
        "targetRms": 0.1,
    });

    let events = jobs.run_all(
        worker,
        project,
        &reference_audio_path,
        &reference_transcript,
        &settings,
    );
    // Save after processing so takes/selection persist.
    let _ = project.autosave();
    Ok(events
        .into_iter()
        .map(|ev| serde_json::to_value(ev).unwrap_or_default())
        .collect())
}

/// Run the next queued job in a single step. Returns the event or `null` if
/// the queue is empty or cancelled. Unlike `jobs_run`, this does not block
/// for the entire queue — the UI can poll it in a loop. The function releases
/// the project lock during the blocking worker call so the UI can still
/// poll events and cancel.
#[tauri::command]
fn jobs_run_next(state: tauri::State<'_, AppState>) -> Result<Option<serde_json::Value>, String> {
    let mut jobs = state.jobs.lock().unwrap();
    if !jobs.has_next() {
        return Ok(None);
    }

    // Pop the job and snapshot needed data under the jobs lock, then release it.
    let job = match jobs.pop_job() {
        Some(j) => j,
        None => return Ok(None),
    };
    // Emit started event and drop jobs lock to avoid deadlock.
    jobs.push_event(domain::job::JobEvent::Started {
        cue_id: job.cue_id.clone(),
    });
    drop(jobs);

    // Snapshot project data under project lock, then release it before the
    // blocking synthesis call.
    let (cue_text, seed, reference_audio_path, reference_transcript, project_dir) = {
        let mut guard = state.project.lock().unwrap();
        let project = guard.as_mut().ok_or_else(|| "no project open".to_string())?;
        let reference = project.reference.as_ref().ok_or_else(|| {
            "no reference voice; build one before generating".to_string()
        })?;
        let cue = project.cues.iter().find(|c| c.id == job.cue_id)
            .ok_or_else(|| format!("cue {} not found", job.cue_id))?;
        let seed = match job.action {
            JobAction::Generate => {
                use rand::Rng;
                rand::thread_rng().gen_range(1..1_000_000_000)
            }
            JobAction::Regenerate { seed } => seed,
        };
        (
            cue.text.clone(),
            seed,
            reference.processed_audio_path.clone(),
            reference.transcript.clone(),
            project.project_dir.clone(),
        )
    };

    let settings = serde_json::json!({
        "nfeStep": 32,
        "cfgStrength": 2.0,
        "swaySamplingCoef": -1.0,
        "speed": 1.0,
        "targetRms": 0.1,
    });

    // Execute the synthesis with only the worker lock held (project lock is
    // released). This is the 30s+ blocking call.
    let ev = {
        let mut worker_guard = state.worker.lock().unwrap();
        let worker = worker_guard.as_mut().ok_or_else(|| "worker not running".to_string())?;

        // Preprocess reference
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
                let ev = domain::job::JobEvent::Failed {
                    cue_id: job.cue_id.clone(),
                    error: e,
                };
                // Re-lock jobs to update state
                let mut j = state.jobs.lock().unwrap();
                j.finish_job();
                j.push_event(ev.clone());
                return Ok(Some(serde_json::to_value(&ev).unwrap_or_default()));
            }
        };

        // Synthesize
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
                "text": cue_text,
                "seed": seed,
                "settings": settings,
            }),
        );
        let result = match synthesize_result {
            Ok(v) => v,
            Err(e) => {
                let ev = domain::job::JobEvent::Failed {
                    cue_id: job.cue_id.clone(),
                    error: e,
                };
                let mut j = state.jobs.lock().unwrap();
                j.finish_job();
                j.push_event(ev.clone());
                return Ok(Some(serde_json::to_value(&ev).unwrap_or_default()));
            }
        };

        let audio_path = result["audioPath"].as_str().unwrap_or("").to_string();
        let duration_ms = result["durationMs"].as_u64().unwrap_or(0);
        let settings_hash = result["settingsHash"].as_str().unwrap_or("").to_string();

        // Move the worker output to a temp name in takes/
        let takes_dir = project_dir.join("takes");
        let _ = std::fs::create_dir_all(&takes_dir);
        let tmp_take = takes_dir.join(format!("tmp-{seed}.wav"));
        let worker_out = std::path::Path::new(&audio_path);
        if worker_out != tmp_take {
            let _ = std::fs::rename(worker_out, &tmp_take);
        }

        // Re-lock project to update state
        let mut guard = state.project.lock().unwrap();
        let project = guard.as_mut().unwrap();
        let cue_idx = project.cues.iter().position(|c| c.id == job.cue_id.clone());
        let counter = cue_idx.map(|idx| project.cues[idx].takes.len() + 1).unwrap_or(1);
        let take_id = format!("take-{seed}-{counter:02}");
        let final_audio = project_dir
            .join("takes")
            .join(format!("{take_id}.wav"));
        // Rename from temp to final name
        let _ = std::fs::rename(&tmp_take, &final_audio);
        let relative_audio = format!("takes/{}", final_audio.file_name().unwrap().to_string_lossy());

        let take = domain::take::Take {
            take_id: take_id.clone(),
            cue_id: job.cue_id.clone(),
            provider: "jaitts-f5tts".to_string(),
            provider_version: "1.1.22".to_string(),
            seed,
            duration_ms,
            settings_hash,
            audio_path: relative_audio,
        };
        if let Some(idx) = cue_idx {
            project.cues[idx].takes.push(take);
            project.cues[idx].selected_take_id = Some(take_id.clone());
            project.cues[idx].dirty = true;
        }
        JobQueue::update_solver(project);
        let _ = project.autosave();

        let ev = domain::job::JobEvent::Completed {
            cue_id: job.cue_id.clone(),
            take_id,
            duration_ms,
        };
        let mut j = state.jobs.lock().unwrap();
        j.finish_job();
        j.push_event(ev.clone());
        ev
    };

    Ok(Some(serde_json::to_value(&ev).unwrap_or_default()))
}

/// Compute waveform peaks for a take audio file.
#[tauri::command]
fn compute_peaks(
    state: tauri::State<'_, AppState>,
    cue_id: String,
    take_id: String,
) -> Result<Vec<domain::media::PeakSegment>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let cue = project
        .cues
        .iter()
        .find(|c| c.id == cue_id)
        .ok_or_else(|| format!("cue {cue_id} not found"))?;
    let take = cue
        .takes
        .iter()
        .find(|t| t.take_id == take_id)
        .ok_or_else(|| format!("take {take_id} not found"))?;
    let audio_path = if std::path::Path::new(&take.audio_path).is_absolute() {
        std::path::PathBuf::from(&take.audio_path)
    } else {
        project.project_dir.join(&take.audio_path)
    };
    domain::media::compute_peaks(&audio_path, 200).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            ping,
            bootstrap_check,
            bootstrap_accept_license,
            bootstrap_download_progress,
            bootstrap_ensure_dirs,
            bootstrap_detect_gpu,
            bootstrap_verify_checksum,
            bootstrap_state,
            bootstrap_run_install,
            bootstrap_find_ffmpeg,
            bootstrap_find_ffprobe,
            worker_spawn,
            worker_configure,
            worker_close,
            worker_ping,
            tts_initialize,
            reference_build_video_segment,
            reference_build_external,
            project_get,
            project_open,
            project_create,
            project_save,
            project_close,
            cue_update_text,
            probe_video_duration,
            export_blocked,
            export_validate,
            export_run,
            solve_timeline,
            take_audio_url,
            jobs_run_next,
            compute_peaks,
            generate_all,
            generate_one,
            regenerate_one,
            job_cancel_after_current,
            job_drain_events,
            take_select,
            take_delete,
            jobs_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
