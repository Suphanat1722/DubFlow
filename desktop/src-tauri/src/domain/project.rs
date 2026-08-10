//! Project Manager for DubFlow projects.
//!
//! A project is a folder with a `.dubflow` extension containing a versioned
//! `project.json`, processed reference, raw takes, and cache.
//!
//! ## Schema versioning
//!
//! `schema_version` is incremented when the project structure changes
//! incompatibly. A migration boundary is enforced at the deserialization layer
//! so that older versions are rejected rather than silently corrupted.

use crate::domain::take::Cue;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Current project schema version.
/// Increment on every backward-incompatible change.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Minimum supported schema version for migration.
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Error types for project operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    Io { message: String },
    Parse { message: String },
    UnsupportedSchema { version: u32, path: String },
    MissingVideo { path: String },
    MissingSrt { path: String },
    CorruptMedia { message: String },
    RelinkFailed { key: String, tried: Vec<String> },
    Worker { message: String },
    InvalidReference { message: String },
    MissingReferenceAudio { path: String },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::Io { message } => write!(f, "IO error: {message}"),
            ProjectError::Parse { message } => write!(f, "parse error: {message}"),
            ProjectError::UnsupportedSchema { version, path } => {
                write!(f, "unsupported schema version {version} at {path}")
            }
            ProjectError::MissingVideo { path } => write!(f, "missing video: {path}"),
            ProjectError::MissingSrt { path } => write!(f, "missing SRT: {path}"),
            ProjectError::CorruptMedia { message } => write!(f, "corrupt media: {message}"),
            ProjectError::RelinkFailed { key, tried } => {
                write!(f, "relink failed for key {key}, tried {tried:?}")
            }
            ProjectError::Worker { message } => write!(f, "worker error: {message}"),
            ProjectError::InvalidReference { message } => {
                write!(f, "invalid reference: {message}")
            }
            ProjectError::MissingReferenceAudio { path } => {
                write!(f, "missing reference audio: {path}")
            }
        }
    }
}

impl std::error::Error for ProjectError {}

/// Reference voice source data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceData {
    pub source: String,
    #[serde(default)]
    pub video_path: String,
    #[serde(default)]
    pub start_ms: u64,
    #[serde(default)]
    pub end_ms: u64,
    #[serde(default)]
    pub external_audio_path: String,
    #[serde(default)]
    pub transcript: String,
    #[serde(default)]
    pub processed_audio_path: String,
}

/// Reference voice source variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSource {
    VideoSegment {
        video_path: String,
        start_ms: u64,
        end_ms: u64,
    },
    ExternalAudio {
        audio_path: String,
    },
}

/// Video path reference with relink key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRef {
    pub path: String,
    pub relink_key: String,
}

/// SRT path reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrtRef {
    pub path: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

/// The on-disk project representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub name: String,
    pub created_at: String,
    pub video: VideoRef,
    pub srt: SrtRef,
    #[serde(default)]
    pub reference: Option<ReferenceData>,
    #[serde(default)]
    pub cues: Vec<Cue>,
    /// Absolute path to the `.dubflow` folder (set at load time, not persisted).
    #[serde(skip)]
    pub project_dir: PathBuf,
    /// Whether the project has unsaved changes. Not persisted.
    #[serde(skip)]
    pub dirty: bool,
}

/// Snapshot of a project for solver input.
#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    pub cues: Vec<Cue>,
    pub video_duration_ms: Option<i64>,
}

impl Project {
    pub fn new(name: String, project_dir: PathBuf, video_path: String, srt_path: String) -> Self {
        let relink_key = compute_relink_key(&video_path);
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            name,
            created_at: iso_now(),
            video: VideoRef {
                path: video_path,
                relink_key,
            },
            srt: SrtRef {
                path: srt_path,
                encoding: "utf-8".to_string(),
            },
            reference: None,
            cues: Vec::new(),
            project_dir,
            dirty: false,
        }
    }

    /// Save the project atomically to `project_dir/project.json`.
    pub fn save(&mut self) -> Result<(), ProjectError> {
        let path = self.project_dir.join("project.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| ProjectError::Parse {
            message: e.to_string(),
        })?;
        let tmp = self.project_dir.join("project.json.tmp");
        std::fs::write(&tmp, &json).map_err(|e| ProjectError::Io {
            message: e.to_string(),
        })?;
        atomic_replace(&tmp, &path).map_err(|e| ProjectError::Io {
            message: format!("atomic save failed: {e}"),
        })?;
        self.dirty = false;
        Ok(())
    }

    /// Autosave the project only when it is dirty.
    pub fn autosave(&mut self) -> Result<(), ProjectError> {
        if self.dirty {
            self.save()
        } else {
            Ok(())
        }
    }

    /// Load a project from a `.dubflow` folder.
    pub fn load(project_dir: PathBuf) -> Result<Self, ProjectError> {
        let path = project_dir.join("project.json");
        let text = std::fs::read_to_string(&path).map_err(|e| ProjectError::Io {
            message: format!("cannot read {path:?}: {e}"),
        })?;
        let mut project: Project =
            serde_json::from_str(&text).map_err(|e| ProjectError::Parse {
                message: format!("{path:?}: {e}"),
            })?;
        project.project_dir = project_dir;
        project.dirty = false;

        if project.schema_version < MIN_SUPPORTED_SCHEMA_VERSION
            || project.schema_version > CURRENT_SCHEMA_VERSION
        {
            return Err(ProjectError::UnsupportedSchema {
                version: project.schema_version,
                path: path.to_string_lossy().to_string(),
            });
        }
        Ok(project)
    }

    /// Validate that referenced media files exist and are accessible.
    pub fn validate_media(&self) -> Result<(), ProjectError> {
        let video = Path::new(&self.video.path);
        if !video.exists() {
            return Err(ProjectError::MissingVideo {
                path: self.video.path.clone(),
            });
        }
        if !video.is_file() {
            return Err(ProjectError::CorruptMedia {
                message: format!("video path is not a file: {}", self.video.path),
            });
        }
        let srt = Path::new(&self.srt.path);
        if !srt.exists() {
            return Err(ProjectError::MissingSrt {
                path: self.srt.path.clone(),
            });
        }
        if !srt.is_file() {
            return Err(ProjectError::CorruptMedia {
                message: format!("SRT path is not a file: {}", self.srt.path),
            });
        }
        Ok(())
    }

    /// Try to relink a missing video by searching alternative paths.
    /// Returns `Ok(())` with the new path if found, or `RelinkFailed`.
    pub fn relink_video(&mut self, candidates: &[String]) -> Result<(), ProjectError> {
        let original_basename = Path::new(&self.video.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        for candidate in candidates {
            let p = Path::new(candidate);
            if p.exists() {
                let candidate_basename = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let key_matches = compute_relink_key(candidate) == self.video.relink_key;
                let basename_matches =
                    !original_basename.is_empty() && candidate_basename == original_basename;
                if key_matches || basename_matches {
                    self.video.path = candidate.to_string();
                    self.dirty = true;
                    return Ok(());
                }
            }
        }
        Err(ProjectError::RelinkFailed {
            key: self.video.relink_key.clone(),
            tried: candidates.to_vec(),
        })
    }

    /// Build a snapshot for the timeline solver.
    pub fn snapshot(&self, video_duration_ms: Option<i64>) -> ProjectSnapshot {
        ProjectSnapshot {
            cues: self.cues.clone(),
            video_duration_ms,
        }
    }
}

/// Compute a relink key for a video file (SHA-256 of the file's canonical path).
pub fn compute_relink_key(path: &str) -> String {
    let canonical = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(&hasher.finalize())
}

fn iso_now() -> String {
    // Rust std cannot format wall-clock time without a timezone crate. For
    // MVP we derive an ISO-8601 UTC timestamp from the Unix epoch; this is
    // deterministic and does not require a third-party datetime dependency.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Replace `target` with `source` atomically.
///
/// On Windows `std::fs::rename` fails when the destination already exists, so
/// use `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.
#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::winbase::{MoveFileExW, MOVEFILE_REPLACE_EXISTING};

    let source_wide: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let target_wide: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(source, target)
}

/// Convert days since 1970-01-01 to a (year, month, day) civil date.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Create a new `.dubflow` project folder.
pub fn create_project(
    parent_dir: &Path,
    name: &str,
    video_path: &str,
    srt_path: &str,
) -> Result<Project, ProjectError> {
    let project_dir = parent_dir.join(format!("{}.dubflow", name));
    std::fs::create_dir_all(&project_dir).map_err(|e| ProjectError::Io {
        message: format!("cannot create project dir {project_dir:?}: {e}"),
    })?;
    std::fs::create_dir_all(project_dir.join("takes")).map_err(|e| ProjectError::Io {
        message: format!("cannot create takes dir: {e}"),
    })?;
    std::fs::create_dir_all(project_dir.join("cache")).map_err(|e| ProjectError::Io {
        message: format!("cannot create cache dir: {e}"),
    })?;
    let mut project = Project::new(
        name.to_string(),
        project_dir,
        video_path.to_string(),
        srt_path.to_string(),
    );
    project.save()?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::take::{CueStatus, Take};
    use std::fs;

    static NEXT_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn tmp_dir() -> PathBuf {
        let n = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dubflow_test_{}_{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_and_load_round_trip() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        let srt = dir.join("test.srt");
        fs::write(&video, b"fake video").unwrap();
        fs::write(&srt, b"fake srt").unwrap();

        let mut project = create_project(
            &dir,
            "my_project",
            video.to_string_lossy().as_ref(),
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();

        // Add a cue with a take
        let mut cue = Cue::new("cue-001".into(), 1, "สวัสดี".into(), 1000, 3000);
        cue.takes.push(Take {
            take_id: "take-001".into(),
            cue_id: "cue-001".into(),
            provider: "jaitts-f5tts".into(),
            provider_version: "1.1.22".into(),
            seed: 42,
            duration_ms: 2100,
            settings_hash: "abc".into(),
            audio_path: "takes/take-001.wav".into(),
        });
        cue.selected_take_id = Some("take-001".into());
        cue.status = CueStatus::Ready;
        project.cues.push(cue);
        project.save().unwrap();

        // Load back
        let loaded = Project::load(project.project_dir.clone()).unwrap();
        assert_eq!(loaded.name, "my_project");
        assert_eq!(loaded.cues.len(), 1);
        assert_eq!(loaded.cues[0].selected_take_id, Some("take-001".into()));
        assert_eq!(loaded.cues[0].selected_take().unwrap().seed, 42);
        assert_eq!(loaded.cues[0].status, CueStatus::Ready);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_video_returns_error() {
        let dir = tmp_dir();
        let srt = dir.join("test.srt");
        fs::write(&srt, b"fake srt").unwrap();

        let project = create_project(
            &dir,
            "missing_video",
            "C:/nonexistent/video.mp4",
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();
        let err = project.validate_media().unwrap_err();
        assert!(matches!(err, ProjectError::MissingVideo { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_srt_returns_error() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        fs::write(&video, b"fake video").unwrap();

        let project = create_project(
            &dir,
            "missing_srt",
            video.to_string_lossy().as_ref(),
            "C:/nonexistent/test.srt",
        )
        .unwrap();
        let err = project.validate_media().unwrap_err();
        assert!(matches!(err, ProjectError::MissingSrt { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let dir = tmp_dir();
        let project_path = dir.join("bad_project.dubflow");
        fs::create_dir_all(&project_path).unwrap();
        let json = r#"{"schemaVersion":99,"name":"bad","createdAt":"2026-08-10T00:00:00Z","video":{"path":"x","relinkKey":"x"},"srt":{"path":"x","encoding":"utf-8"}}"#;
        fs::write(project_path.join("project.json"), json).unwrap();

        let err = Project::load(project_path).unwrap_err();
        assert!(matches!(
            err,
            ProjectError::UnsupportedSchema { version: 99, .. }
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relink_video_finds_candidate() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        fs::write(&video, b"fake video").unwrap();
        let srt = dir.join("test.srt");
        fs::write(&srt, b"fake srt").unwrap();

        let mut project = create_project(
            &dir,
            "relink_test",
            video.to_string_lossy().as_ref(),
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();

        // Move the video to a new location
        // Rename to a different filename in a different location
        let new_video = dir.join("renamed_location").join("different_name.mp4");
        fs::create_dir_all(new_video.parent().unwrap()).unwrap();
        fs::copy(&video, &new_video).unwrap();

        // A candidate with a different basename and different canonical path
        // does not match the relink key, so relinking fails with a typed error.
        let err = project
            .relink_video(&[new_video.to_string_lossy().to_string()])
            .unwrap_err();
        assert!(matches!(err, ProjectError::RelinkFailed { .. }));
        assert_eq!(project.video.path, video.to_string_lossy());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relink_video_matches_by_basename() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        fs::write(&video, b"fake video").unwrap();
        let srt = dir.join("test.srt");
        fs::write(&srt, b"fake srt").unwrap();

        let mut project = create_project(
            &dir,
            "relink_basename",
            video.to_string_lossy().as_ref(),
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();

        // Move the file into another folder, keeping the same basename.
        let new_dir = dir.join("new_location");
        fs::create_dir_all(&new_dir).unwrap();
        let new_video = new_dir.join("test.mp4");
        fs::rename(&video, &new_video).unwrap();
        assert!(project
            .relink_video(&[new_video.to_string_lossy().to_string()])
            .is_ok());
        assert_eq!(project.video.path, new_video.to_string_lossy());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_media_detected() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        let srt = dir.join("test.srt");
        // Create a directory with the same name as the "video file"
        std::fs::create_dir_all(&video).unwrap();
        fs::write(&srt, b"fake srt").unwrap();

        let project = create_project(
            &dir,
            "corrupt_media",
            video.to_string_lossy().as_ref(),
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();
        let err = project.validate_media().unwrap_err();
        assert!(matches!(err, ProjectError::CorruptMedia { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_skips_clean_project() {
        let dir = tmp_dir();
        let video = dir.join("test.mp4");
        let srt = dir.join("test.srt");
        fs::write(&video, b"fake video").unwrap();
        fs::write(&srt, b"fake srt").unwrap();

        let mut project = create_project(
            &dir,
            "autosave_clean",
            video.to_string_lossy().as_ref(),
            srt.to_string_lossy().as_ref(),
        )
        .unwrap();
        let project_path = project.project_dir.join("project.json");
        let before = fs::metadata(&project_path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        project.autosave().unwrap();
        let after = fs::metadata(&project_path).unwrap().modified().unwrap();
        assert_eq!(
            before, after,
            "clean project should not rewrite project.json"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn golden_persistence_round_trip_preserves_take_selection_and_solver() {
        // Creates a project with takes, saves, reloads, solves both timelines
        // and verifies the results are identical.
        let dir = tmp_dir();
        let video = dir.join("video.mp4");
        let srt_file = dir.join("subs.srt");
        fs::write(&video, b"fake video").unwrap();
        fs::write(&srt_file, b"fake srt").unwrap();

        let mut project = create_project(
            &dir,
            "golden_save",
            video.to_string_lossy().as_ref(),
            srt_file.to_string_lossy().as_ref(),
        )
        .unwrap();

        // Build 3 cues with selected takes
        for i in 0..3 {
            let cue_id = format!("cue-{:03}", i + 1);
            let base = i as i64 * 3000;
            let mut cue = Cue::new(
                cue_id.clone(),
                (i + 1) as u32,
                format!("Cue {}", i + 1),
                base,
                base + 2000,
            );
            let take_id = format!("take-{:03}", i + 1);
            cue.takes.push(Take {
                take_id: take_id.clone(),
                cue_id: cue_id.clone(),
                provider: "jaitts-f5tts".into(),
                provider_version: "1.1.22".into(),
                seed: 100 + i as u64,
                duration_ms: 1800 + (i * 100) as u64,
                settings_hash: format!("hash-{}", i),
                audio_path: format!("takes/{}.wav", take_id),
            });
            cue.selected_take_id = Some(take_id);
            cue.status = CueStatus::Ready;
            project.cues.push(cue);
        }

        // Solve the timeline before save
        let solver_inputs: Vec<_> = project
            .cues
            .iter()
            .map(|c| {
                crate::domain::timeline::SolverInput::new(
                    c.id.clone(),
                    c.srt_start_ms,
                    c.srt_end_ms,
                    c.selected_duration_ms(),
                )
            })
            .collect();
        let result_before = crate::domain::timeline::solve(&solver_inputs, Some(60_000));

        project.save().unwrap();

        // Reload
        let loaded = Project::load(project.project_dir.clone()).unwrap();

        // Verify take selection preserved
        assert_eq!(loaded.cues.len(), 3);
        for (i, cue) in loaded.cues.iter().enumerate() {
            let expected_take_id = format!("take-{:03}", i + 1);
            assert_eq!(
                cue.selected_take_id,
                Some(expected_take_id),
                "cue {} take selection lost after save",
                i + 1
            );
            assert_eq!(cue.status, CueStatus::Ready);
            assert_eq!(cue.selected_duration_ms(), 1800 + (i * 100) as u64);
        }

        // Solve the timeline after reload
        let solver_inputs_after: Vec<_> = loaded
            .cues
            .iter()
            .map(|c| {
                crate::domain::timeline::SolverInput::new(
                    c.id.clone(),
                    c.srt_start_ms,
                    c.srt_end_ms,
                    c.selected_duration_ms(),
                )
            })
            .collect();
        let result_after = crate::domain::timeline::solve(&solver_inputs_after, Some(60_000));

        assert_eq!(result_before.cues.len(), result_after.cues.len());
        for (before, after) in result_before.cues.iter().zip(result_after.cues.iter()) {
            assert_eq!(before.cue_id, after.cue_id);
            assert_eq!(before.render_start_ms, after.render_start_ms);
            assert_eq!(before.render_end_ms, after.render_end_ms);
            assert!((before.speed - after.speed).abs() < 0.001);
            assert_eq!(before.status, after.status);
        }
        assert_eq!(result_before.export_blocked, result_after.export_blocked);
        assert_eq!(
            result_before.total_render_end_ms,
            result_after.total_render_end_ms
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

/// Hex encoding helper (avoids adding the `hex` crate).
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
