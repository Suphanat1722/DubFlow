pub mod domain;

use std::path::PathBuf;
use std::sync::Mutex;

use domain::job::{self, Job, JobAction, JobQueue};
use domain::project::{Project, ProjectError};
use domain::reference;
use domain::worker::WorkerClient;

/// App state shared between Tauri commands.
pub struct AppState {
    /// The currently open project (if any).
    pub project: Mutex<Option<Project>>,
    /// The Python worker sidecar.
    pub worker: Mutex<Option<WorkerClient>>,
    /// Sequential generation job queue.
    pub jobs: Mutex<JobQueue>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: Mutex::new(None),
            worker: Mutex::new(None),
            jobs: Mutex::new(JobQueue::new()),
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
    Ok(guard
        .as_ref()
        .map(|p| serde_json::to_value(p).unwrap_or_default()))
}

#[tauri::command]
fn project_open(
    state: tauri::State<'_, AppState>,
    project_dir: String,
) -> Result<serde_json::Value, String> {
    let mut guard = state.project.lock().unwrap();
    let project = Project::load(PathBuf::from(project_dir)).map_err(project_error_to_command)?;
    let value = serde_json::to_value(&project).map_err(|e| e.to_string())?;
    *guard = Some(project);
    Ok(value)
}

/// Queue Generate All for every cue in the project.
#[tauri::command]
fn generate_all(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let mut jobs = state.jobs.lock().unwrap();
    let project = state.project.lock().unwrap();
    let Some(project) = project.as_ref() else {
        return Err("no project open".to_string());
    };
    let jobs_vec: Vec<Job> = project
        .cues
        .iter()
        .map(|c| Job {
            cue_id: c.id.clone(),
            action: JobAction::Generate,
        })
        .collect();
    let count = jobs_vec.len();
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            worker_spawn,
            worker_configure,
            worker_close,
            worker_ping,
            tts_initialize,
            reference_build_video_segment,
            reference_build_external,
            project_get,
            project_open,
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
