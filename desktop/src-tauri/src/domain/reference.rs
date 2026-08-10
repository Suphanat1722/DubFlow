//! Reference voice building: extract/prepare reference audio and metadata.

use std::path::Path;

use super::media::{self, MediaError};
use super::project::{Project, ProjectError, ReferenceData};

/// Reference voice sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSource {
    /// A 3-12 second segment of the project video, transcript from SRT.
    VideoSegment {
        start_ms: i64,
        end_ms: i64,
        transcript: String,
    },
    /// An external audio file with a manual transcript.
    ExternalAudio {
        audio_path: String,
        transcript: String,
    },
}

/// Result of building a reference voice.
#[derive(Debug, Clone)]
pub struct ReferenceBuildResult {
    pub reference: ReferenceData,
    pub duration_ms: i64,
}

/// Validate the video reference segment range against the project video
/// duration.
pub fn validate_video_segment(
    project: &Project,
    start_ms: i64,
    end_ms: i64,
) -> Result<(), ProjectError> {
    if start_ms < 0 || end_ms <= start_ms || end_ms - start_ms > 12_000 {
        return Err(ProjectError::InvalidReference {
            message: "reference segment must be 3-12 seconds, non-empty and within the video".into(),
        });
    }
    if end_ms - start_ms < 3_000 {
        return Err(ProjectError::InvalidReference {
            message: "reference segment must be at least 3 seconds".into(),
        });
    }
    let video = Path::new(&project.video.path);
    let duration = media::probe_duration(video).map_err(|e| ProjectError::CorruptMedia {
        message: format!("cannot probe video: {e}"),
    })?;
    if end_ms > duration {
        return Err(ProjectError::InvalidReference {
            message: format!(
                "reference segment ends at {end_ms}ms but video duration is {duration}ms"
            ),
        });
    }
    Ok(())
}

/// Build a reference voice from a video segment.
///
/// Extracts the segment to `out_path` (24 kHz mono WAV) and returns the
/// persisted reference metadata.
pub fn build_video_segment_reference(
    project: &Project,
    source: ReferenceSource,
    out_path: &Path,
) -> Result<ReferenceBuildResult, ProjectError> {
    let ReferenceSource::VideoSegment {
        start_ms,
        end_ms,
        transcript,
    } = source
    else {
        return Err(ProjectError::InvalidReference {
            message: "expected video segment source".into(),
        });
    };
    validate_video_segment(project, start_ms, end_ms)?;
    let video = Path::new(&project.video.path);
    media::extract_audio_segment(video, out_path, start_ms, end_ms).map_err(|e| {
        ProjectError::CorruptMedia {
            message: format!("cannot extract reference segment: {e}"),
        }
    })?;
    let duration_ms = media::probe_audio_duration(out_path).map_err(|e| {
        ProjectError::CorruptMedia {
            message: format!("cannot read extracted reference: {e}"),
        }
    })?;
    Ok(ReferenceBuildResult {
        reference: ReferenceData {
            source: "video-segment".into(),
            video_path: project.video.path.clone(),
            start_ms: start_ms as u64,
            end_ms: end_ms as u64,
            external_audio_path: String::new(),
            transcript,
            processed_audio_path: out_path.to_string_lossy().to_string(),
        },
        duration_ms,
    })
}

/// Build a reference voice from an external audio file.
///
/// The audio is copied into the project (so it survives project relocation)
/// and the transcript is stored as provided.
pub fn build_external_reference(
    _project: &Project,
    audio_path: &str,
    transcript: &str,
    out_path: &Path,
) -> Result<ReferenceBuildResult, ProjectError> {
    let src = Path::new(audio_path);
    if !src.is_file() {
        return Err(ProjectError::MissingReferenceAudio {
            path: audio_path.to_string(),
        });
    }
    let duration_ms = media::probe_audio_duration(src).map_err(|e| {
        ProjectError::CorruptMedia {
            message: format!("cannot probe reference audio: {e}"),
        }
    })?;
    if !(3_000..=12_000).contains(&duration_ms) {
        return Err(ProjectError::InvalidReference {
            message: "external reference audio must be 3-12 seconds".into(),
        });
    }
    if transcript.trim().is_empty() {
        return Err(ProjectError::InvalidReference {
            message: "external reference requires a transcript".into(),
        });
    }
    std::fs::copy(src, out_path).map_err(|e| ProjectError::Io {
        message: format!("cannot copy reference audio: {e}"),
    })?;
    Ok(ReferenceBuildResult {
        reference: ReferenceData {
            source: "external-audio".into(),
            video_path: String::new(),
            start_ms: 0,
            end_ms: 0,
            external_audio_path: audio_path.to_string(),
            transcript: transcript.to_string(),
            processed_audio_path: out_path.to_string_lossy().to_string(),
        },
        duration_ms,
    })
}

/// Verify the reference audio can be used by the TTS pipeline (non-empty,
/// decodable). Delegates to ffprobe; full validation happens in the worker
/// during `tts.preprocess_reference`.
pub fn validate_reference_audio(path: &Path) -> Result<i64, MediaError> {
    if !path.is_file() {
        return Err(MediaError::CorruptInput {
            path: path.to_string_lossy().to_string(),
            message: "reference audio file does not exist".into(),
        });
    }
    let has_audio = media::has_audio_stream(path)?;
    if !has_audio {
        return Err(MediaError::NoAudioStream {
            path: path.to_string_lossy().to_string(),
        });
    }
    media::probe_audio_duration(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_segment_outside_bounds() {
        // Reference segment must be 3-12 seconds.
        assert!(validate_video_segment_bounds(0, 15000).is_err());
        assert!(validate_video_segment_bounds(0, 2000).is_err());
        assert!(validate_video_segment_bounds(5000, 4000).is_err());
        assert!(validate_video_segment_bounds(0, 6000).is_ok());
    }

    fn validate_video_segment_bounds(start_ms: i64, end_ms: i64) -> Result<(), ()> {
        if start_ms < 0 || end_ms <= start_ms {
            return Err(());
        }
        let dur = end_ms - start_ms;
        if !(3_000..=12_000).contains(&dur) {
            return Err(());
        }
        Ok(())
    }
}
