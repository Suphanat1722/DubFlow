//! Media operations via FFmpeg/FFprobe.
//!
//! All operations run as subprocesses. The caller is responsible for passing
//! the path to the FFmpeg binary (or relying on PATH). Results are parsed
//! from structured output (JSON via ffprobe) or checked via exit code.

use std::path::Path;
use std::process::Command;

/// Media operation errors.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaError {
    FfmpegNotFound { message: String },
    FfprobeNotFound { message: String },
    ProbeFailed { path: String, message: String },
    ExtractionFailed { message: String },
    StretchFailed { message: String },
    NormalizationFailed { message: String },
    InvalidSpeed { speed: f64 },
    CorruptInput { path: String, message: String },
    NoAudioStream { path: String },
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaError::FfmpegNotFound { message } => write!(f, "ffmpeg not found: {message}"),
            MediaError::FfprobeNotFound { message } => write!(f, "ffprobe not found: {message}"),
            MediaError::ProbeFailed { path, message } => {
                write!(f, "probe failed for {path}: {message}")
            }
            MediaError::ExtractionFailed { message } => write!(f, "extraction failed: {message}"),
            MediaError::StretchFailed { message } => write!(f, "stretch failed: {message}"),
            MediaError::NormalizationFailed { message } => {
                write!(f, "normalization failed: {message}")
            }
            MediaError::InvalidSpeed { speed } => write!(f, "invalid speed {speed} (must be 1.0-1.25)"),
            MediaError::CorruptInput { path, message } => {
                write!(f, "corrupt input {path}: {message}")
            }
            MediaError::NoAudioStream { path } => {
                write!(f, "no audio stream in {path}")
            }
        }
    }
}

impl std::error::Error for MediaError {}

/// Probe the duration (ms) of a media file via ffprobe.
pub fn probe_duration(path: &Path) -> Result<i64, MediaError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: format!("cannot execute ffprobe: {e}"),
        })?;
    if !output.status.success() {
        return Err(MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: format!("cannot parse ffprobe output: {e}"),
        })?;
    let duration_sec = parsed["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    Ok((duration_sec * 1000.0).round() as i64)
}

/// Probe the audio duration (ms) of a WAV file.
pub fn probe_audio_duration(path: &Path) -> Result<i64, MediaError> {
    probe_duration(path)
}

/// Extract an audio segment from a video file.
///
/// Output is a 24 kHz mono WAV (compatible with the JaiTTS reference
/// preprocessing pipeline).
pub fn extract_audio_segment(
    input: &Path,
    output: &Path,
    start_ms: i64,
    end_ms: i64,
) -> Result<(), MediaError> {
    let duration_sec = (end_ms - start_ms) as f64 / 1000.0;
    let start_sec = start_ms as f64 / 1000.0;
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss",
            &format!("{:.3}", start_sec),
            "-i",
        ])
        .arg(input)
        .args([
            "-t",
            &format!("{:.3}", duration_sec),
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "24000",
            "-ac",
            "1",
        ])
        .arg(output)
        .status()
        .map_err(|e| MediaError::ExtractionFailed {
            message: format!("cannot execute ffmpeg: {e}"),
        })?;
    if !status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!("ffmpeg exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Pitch-preserving time-stretch using rubberband (via FFmpeg `rubberband`).
///
/// `speed` must be between 1.0 and 1.25. Output is a 48 kHz mono WAV.
/// The atempo filter alone changes pitch; rubberband preserves it.
pub fn pitch_preserving_stretch(
    input: &Path,
    output: &Path,
    speed: f64,
) -> Result<(), MediaError> {
    if !(1.0..=1.25).contains(&speed) {
        return Err(MediaError::InvalidSpeed { speed });
    }
    let tempo = format!("{:.4}", speed);
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args([
            "-af",
            &format!("rubberband=tempo={tempo}:pitch=1"),
            "-ar",
            "48000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(output)
        .status()
        .map_err(|e| MediaError::StretchFailed {
            message: format!("cannot execute ffmpeg: {e}"),
        })?;
    if !status.success() {
        return Err(MediaError::StretchFailed {
            message: format!("ffmpeg exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Normalize audio to a target loudness and true-peak limit.
///
/// Defaults: -18 LUFS integrated, true peak <= -1.5 dBTP.
/// Uses a two-pass FFmpeg `loudnorm` (EBU R128): first pass measures
/// integrated/true-peak/LRA, second pass applies the linear normalization
/// with the measured values so the output actually lands on target.
pub fn normalize_loudness(
    input: &Path,
    output: &Path,
    lufs: f64,
    true_peak_db: f64,
) -> Result<(), MediaError> {
    let measured = measure_loudness(input)?;

    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args([
            "-af",
            &format!(
                "loudnorm=I={lufs}:TP={true_peak_db}:LRA=7:measured_I={mi}:measured_TP={mtp}:measured_LRA={mlra}:measured_thresh={mthresh}:offset={off}:linear=true:print_format=summary",
                lufs = lufs,
                true_peak_db = true_peak_db,
                mi = measured.input_i,
                mtp = measured.true_peak,
                mlra = measured.lra,
                mthresh = measured.thresh,
                off = measured.target_offset,
            ),
            "-ar",
            "48000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(output)
        .status()
        .map_err(|e| MediaError::NormalizationFailed {
            message: format!("cannot execute ffmpeg: {e}"),
        })?;
    if !status.success() {
        return Err(MediaError::NormalizationFailed {
            message: format!("ffmpeg exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Measured loudness values from the first pass.
#[derive(Debug, Clone, Copy, Default)]
struct LoudnessMeasurement {
    input_i: f64,
    true_peak: f64,
    lra: f64,
    thresh: f64,
    target_offset: f64,
}

fn measure_loudness(input: &Path) -> Result<LoudnessMeasurement, MediaError> {
    let output = Command::new("ffmpeg")
        .args(["-i"])
        .arg(input)
        .args([
            "-af",
            "loudnorm=I=-18:TP=-1.5:LRA=7:print_format=json",
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| MediaError::NormalizationFailed {
            message: format!("cannot execute ffmpeg measure pass: {e}"),
        })?;
    if !output.status.success() {
        return Err(MediaError::NormalizationFailed {
            message: format!(
                "ffmpeg measure pass failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    // loudnorm prints a JSON block on stderr.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json_start = stderr.find('{').ok_or_else(|| {
        MediaError::NormalizationFailed {
            message: "loudnorm did not produce JSON output".to_string(),
        }
    })?;
    let json_end = stderr.rfind('}').ok_or_else(|| {
        MediaError::NormalizationFailed {
            message: "loudnorm output truncated".to_string(),
        }
    })?;
    let json_str = &stderr[json_start..=json_end];
    let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        MediaError::NormalizationFailed {
            message: format!("cannot parse loudnorm output: {e}"),
        }
    })?;
    Ok(LoudnessMeasurement {
        input_i: parsed["input_i"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        true_peak: parsed["true_peak"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        lra: parsed["input_lra"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        thresh: parsed["input_thresh"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        target_offset: parsed["target_offset"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
    })
}

/// Check whether a file has an audio stream via ffprobe.
pub fn has_audio_stream(path: &Path) -> Result<bool, MediaError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: format!("cannot execute ffprobe: {e}"),
        })?;
    if !output.status.success() {
        return Err(MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: format!("cannot parse ffprobe output: {e}"),
        })?;
    let streams = parsed["streams"].as_array().ok_or_else(|| {
        MediaError::ProbeFailed {
            path: path.to_string_lossy().to_string(),
            message: "no streams array in ffprobe output".to_string(),
        }
    })?;
    Ok(streams.iter().any(|s| s["codec_type"] == "audio"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_error_display() {
        let err = MediaError::InvalidSpeed { speed: 2.0 };
        assert!(err.to_string().contains("1.0-1.25"));
        let err2 = MediaError::NoAudioStream {
            path: "test.mp4".into(),
        };
        assert!(err2.to_string().contains("no audio stream"));
    }
}
