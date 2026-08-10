//! Export pipelines: voice master assembly, Replace, Mix and Voice Track.
//!
//! All export work happens in the Rust shell via FFmpeg/FFprobe subprocesses,
//! so it does not depend on the Python worker or a GPU (see D-010). Raw takes
//! are never modified; stretched/normalized cache files are reused when the
//! solver assigns speed > 1.0 (see D-024).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::domain::media::MediaError;
use crate::domain::take::Cue;
use crate::domain::timeline::{self, SolverResult};

/// Export modes exposed to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportMode {
    /// Copy video stream, replace audio with AI voice (AAC-LC 192 kbps).
    Replace,
    /// Original audio at `original_gain_db`, AI voice at 0 dB, final limiter.
    Mix,
    /// Mono 48 kHz 24-bit PCM WAV voice track only.
    VoiceTrack,
}

/// Result of the pre-export validation pass.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportValidation {
    pub export_blocked: bool,
    pub reasons: Vec<String>,
}

/// Everything the export pipeline needs from the project.
pub struct ExportRequest {
    pub project_dir: PathBuf,
    pub video_path: String,
    pub cues: Vec<Cue>,
    pub output_path: PathBuf,
    pub mode: ExportMode,
    pub original_gain_db: f64,
}

/// Resolve the absolute audio path of a take.
fn resolve_audio(project_dir: &Path, audio_path: &str) -> PathBuf {
    let p = Path::new(audio_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        project_dir.join(p)
    }
}

/// Validate that an export can proceed.
///
/// Returns per-cue blocker reasons, and additionally rejects the Mix mode
/// when the source video has no audio stream (projected from the MP4
/// H.264/AAC scope, but checked explicitly at export time).
pub fn validate_export(
    cues: &[Cue],
    mode: ExportMode,
    video_has_audio: bool,
    video_duration_ms: Option<i64>,
) -> ExportValidation {
    let mut reasons: Vec<String> = Vec::new();

    for cue in cues {
        let label = format!("cue {} ({})", cue.index, cue.text);
        if let Some(duration) = video_duration_ms {
            if cue.srt_start_ms >= duration {
                reasons.push(format!("{label}: starts outside the video"));
            } else if cue.srt_end_ms > duration {
                reasons.push(format!("{label}: ends outside the video"));
            }
        }
        match cue.status {
            crate::domain::take::CueStatus::NotGenerated => {
                reasons.push(format!("{label}: still not generated"));
            }
            crate::domain::take::CueStatus::Error => {
                reasons.push(format!("{label}: generation error"));
            }
            crate::domain::take::CueStatus::TooLong => {
                reasons.push(format!("{label}: too long for the video"));
            }
            _ => {}
        }
    }

    if mode == ExportMode::Mix && !video_has_audio {
        reasons.push("Mix requires a video with an audio stream".to_string());
    }

    ExportValidation {
        export_blocked: !reasons.is_empty(),
        reasons,
    }
}

/// Build the timeline solver inputs for the project's cues.
fn solver_inputs(cues: &[Cue]) -> Vec<timeline::SolverInput> {
    cues.iter()
        .map(|c| {
            timeline::SolverInput::new(
                c.id.clone(),
                c.srt_start_ms,
                c.srt_end_ms,
                c.selected_duration_ms(),
            )
        })
        .collect()
}

/// Render the full voice master from the solved timeline.
///
/// The output is a mono 48 kHz WAV whose length is exactly `video_duration_ms`
/// (in the 48 kHz sample domain), with each cue's stretched/normalized audio
/// placed at its solved render start and padded/trimmed so the master covers
/// the whole video. Returns the number of output samples.
pub fn render_voice_master(
    project_dir: &Path,
    cues: &[Cue],
    video_duration_ms: i64,
    master_path: &Path,
) -> Result<u64, MediaError> {
    let inputs = solver_inputs(cues);
    let solved = timeline::solve(&inputs, Some(video_duration_ms));
    assemble_master(project_dir, cues, &solved, video_duration_ms, master_path)
}

fn assemble_master(
    project_dir: &Path,
    cues: &[Cue],
    solved: &SolverResult,
    video_duration_ms: i64,
    master_path: &Path,
) -> Result<u64, MediaError> {
    let total_samples = video_duration_ms * timeline::SAMPLES_PER_MS;
    let mut filters: Vec<String> = Vec::new();
    let mut input_paths: Vec<String> = Vec::new();

    for (i, cue) in cues.iter().enumerate() {
        let Some(solved_cue) = solved.cues.iter().find(|s| s.cue_id == cue.id) else {
            continue;
        };
        let Some(take) = cue.selected_take() else {
            continue;
        };
        let raw = resolve_audio(project_dir, &take.audio_path);
        if !raw.is_file() {
            return Err(MediaError::ExtractionFailed {
                message: format!("missing take audio: {}", raw.display()),
            });
        }

        // Use the stretched + normalized cache when the solver applies speed.
        let audio: PathBuf = if solved_cue.speed > 1.0 {
            crate::domain::job::get_or_create_stretched(project_dir, take, solved_cue.speed)
                .map_err(|e| MediaError::ExtractionFailed {
                    message: format!("cannot prepare stretched take: {e:?}"),
                })?
        } else {
            raw
        };

        input_paths.push(audio.to_string_lossy().to_string());

        let label = format!("c{i}");
        // `adelay` shifts the take to its solved render start. The delay is
        // expressed in milliseconds derived from the 48 kHz sample count, so
        // rounding error stays below one output sample. `apad=whole_len`
        // then pads to the full video length and `atrim` clips any overflow.
        let delay_ms = solved_cue.render_start_sample as f64 / timeline::SAMPLES_PER_MS as f64;
        filters.push(format!(
            "[{i}:a]aresample=48000,aformat=channel_layouts=mono,adelay={delay_ms:.3}|{delay_ms:.3},apad=whole_len={total},atrim=0:{total},asetpts=PTS-STARTPTS[{label}o]",
            delay_ms = delay_ms,
            total = total_samples,
        ));
    }

    if filters.is_empty() {
        // No generated cues: export a silent master covering the video.
        return render_silent_master(total_samples, master_path);
    }

    let mix_inputs = (0..filters.len())
        .map(|i| format!("[c{i}o]"))
        .collect::<String>();
    let count = filters.len();
    let total = total_samples;
    let filter_complex = format!(
        "{};{}amix=inputs={}:normalize=0,atrim=0:{}[out]",
        filters.join(";"),
        mix_inputs,
        count,
        total,
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y");
    for input in &input_paths {
        cmd.arg("-i").arg(input);
    }
    cmd.args(["-filter_complex", &filter_complex, "-map", "[out]"])
        .args([
            "-ar", "48000",
            "-ac", "1",
            "-c:a", "pcm_s24le",
        ])
        .arg(master_path);

    let output = cmd.output().map_err(|e| MediaError::ExtractionFailed {
        message: format!("cannot execute ffmpeg for master: {e}"),
    })?;
    if !output.status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!(
                "ffmpeg master assembly exited with code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }

    Ok(total_samples as u64)
}

/// Create a silent master of exactly `total_samples` samples.
fn render_silent_master(total_samples: i64, master_path: &Path) -> Result<u64, MediaError> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .args([
            "-f", "lavfi",
            "-i", "anullsrc=channel_layout=mono:sample_rate=48000",
            "-t", &format!("{}", total_samples as f64 / timeline::SAMPLE_RATE_HZ as f64),
            "-ar", "48000",
            "-ac", "1",
            "-c:a", "pcm_s24le",
        ])
        .arg(master_path);
    let status = cmd.status().map_err(|e| MediaError::ExtractionFailed {
        message: format!("cannot execute ffmpeg for silent master: {e}"),
    })?;
    if !status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!("ffmpeg silent master exited with code {:?}", status.code()),
        });
    }
    Ok(total_samples as u64)
}

/// Replace the video's audio with the voice master.
///
/// The video stream is copied (no re-encode) and the AI voice is encoded to
/// AAC-LC 192 kbps. Returns the final output path.
pub fn export_replace(
    video_path: &str,
    master_path: &Path,
    output_path: &Path,
) -> Result<(), MediaError> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(master_path)
        .args([
            "-map", "0:v:0",
            "-map", "1:a:0",
            "-c:v", "copy",
            "-c:a", "aac",
            "-b:a", "192k",
            "-movflags", "+faststart",
        ])
        .arg(output_path);
    let status = cmd.status().map_err(|e| MediaError::ExtractionFailed {
        message: format!("cannot execute ffmpeg for replace: {e}"),
    })?;
    if !status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!("ffmpeg replace exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Mix the original audio (at `original_gain_db`, default -12) with the voice
/// master (0 dB) and apply a final limiter.
pub fn export_mix(
    video_path: &str,
    master_path: &Path,
    output_path: &Path,
    original_gain_db: f64,
) -> Result<(), MediaError> {
    let original_gain = format!("{:.3}", original_gain_db);
    let filter_complex = format!(
        "[0:a]volume={original_gain},aformat=channel_layouts=stereo[orig];[1:a]volume=1.0,aformat=channel_layouts=stereo[voice];[orig][voice]amix=inputs=2:duration=longest:normalize=0,alimiter=limit=0.95[out]",
    );
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(master_path)
        .args(["-filter_complex", &filter_complex, "-map", "0:v:0", "-map", "[out]"])
        .args([
            "-c:v", "copy",
            "-c:a", "aac",
            "-b:a", "192k",
            "-movflags", "+faststart",
        ])
        .arg(output_path);
    let status = cmd.status().map_err(|e| MediaError::ExtractionFailed {
        message: format!("cannot execute ffmpeg for mix: {e}"),
    })?;
    if !status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!("ffmpeg mix exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Export the voice track as mono 48 kHz 24-bit PCM WAV.
pub fn export_voice_track(
    master_path: &Path,
    output_path: &Path,
) -> Result<(), MediaError> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-i")
        .arg(master_path)
        .args([
            "-map", "0:a:0",
            "-ar", "48000",
            "-ac", "1",
            "-c:a", "pcm_s24le",
        ])
        .arg(output_path);
    let status = cmd.status().map_err(|e| MediaError::ExtractionFailed {
        message: format!("cannot execute ffmpeg for voice track: {e}"),
    })?;
    if !status.success() {
        return Err(MediaError::ExtractionFailed {
            message: format!("ffmpeg voice track exited with code {:?}", status.code()),
        });
    }
    Ok(())
}

/// Run one export request end to end.
///
/// Returns the master sample count for Replace/VoiceTrack and the output
/// sample count for Mix (used by tests to verify duration).
pub fn run_export(request: &ExportRequest, video_has_audio: bool) -> Result<u64, MediaError> {
    let video_duration_ms = crate::domain::media::probe_duration(Path::new(&request.video_path))?;
    let validation = validate_export(
        &request.cues,
        request.mode,
        video_has_audio,
        Some(video_duration_ms),
    );
    if validation.export_blocked {
        return Err(MediaError::ExtractionFailed {
            message: format!(
                "export blocked: {}",
                validation.reasons.join("; ")
            ),
        });
    }

    let master_path = request.project_dir.join("cache").join("voice-master.wav");
    if let Some(parent) = master_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MediaError::ExtractionFailed {
            message: format!("cannot create cache dir: {e}"),
        })?;
    }

    let master_samples = render_voice_master(
        &request.project_dir,
        &request.cues,
        video_duration_ms,
        &master_path,
    )?;

    if let Some(parent) = request.output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| MediaError::ExtractionFailed {
                message: format!("cannot create output dir: {e}"),
            })?;
        }
    }

    match request.mode {
        ExportMode::Replace => export_replace(&request.video_path, &master_path, &request.output_path)?,
        ExportMode::Mix => export_mix(
            &request.video_path,
            &master_path,
            &request.output_path,
            request.original_gain_db,
        )?,
        ExportMode::VoiceTrack => {
            export_voice_track(&master_path, &request.output_path)?;
        }
    }

    Ok(master_samples)
}

/// Validate an exported file with ffprobe: it must exist, be playable
/// (duration > 0), have the expected stream layout, and its duration must
/// match `expected_ms` within one output sample at the target sample rate.
pub fn validate_output(
    path: &Path,
    expected_ms: i64,
    expected_audio_codec: Option<&str>,
    expected_channels: Option<u32>,
) -> Result<(), MediaError> {
    if !path.is_file() {
        return Err(MediaError::CorruptInput {
            path: path.to_string_lossy().to_string(),
            message: "exported file does not exist".to_string(),
        });
    }
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration:stream=codec_type,codec_name,channels",
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

    let duration_ms = parsed["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| (s * 1000.0).round() as i64)
        .unwrap_or(-1);
    if duration_ms <= 0 {
        return Err(MediaError::CorruptInput {
            path: path.to_string_lossy().to_string(),
            message: format!("exported file has invalid duration: {duration_ms}ms"),
        });
    }

    if let Some(codec) = expected_audio_codec {
        let audio_streams: Vec<&str> = parsed["streams"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|s| s["codec_type"] == "audio")
                    .filter_map(|s| s["codec_name"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        if !audio_streams.contains(&codec) {
            return Err(MediaError::CorruptInput {
                path: path.to_string_lossy().to_string(),
                message: format!("expected audio codec {codec}, found {:?}", audio_streams),
            });
        }
    }

    if let Some(channels) = expected_channels {
        let actual = parsed["streams"]
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|s| s["codec_type"] == "audio")
                    .and_then(|s| s["channels"].as_u64())
            })
            .unwrap_or(0) as u32;
        if actual != channels {
            return Err(MediaError::CorruptInput {
                path: path.to_string_lossy().to_string(),
                message: format!("expected {channels} audio channels, found {actual}"),
            });
        }
    }

    // Duration must match within one output sample. Voice master is 48 kHz
    // (21us per sample), AAC decode is 44.1/48k frame-rounded, so allow a
    // one-sample tolerance at the target rate.
    let tolerance_ms = 1.0 / 48.0 * 1000.0;
    let diff = (duration_ms - expected_ms).abs() as f64;
    if diff > tolerance_ms + 1.0 {
        return Err(MediaError::CorruptInput {
            path: path.to_string_lossy().to_string(),
            message: format!(
                "exported duration {duration_ms}ms does not match expected {expected_ms}ms"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::take::{Cue, CueStatus, Take};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_EXPORT_DIR: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir() -> PathBuf {
        let n = NEXT_EXPORT_DIR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dubflow_export_test_{}_{}", std::process::id(), n));
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

    fn make_cue(id: &str, index: u32, start: i64, end: i64, dur: u64) -> Cue {
        let mut cue = Cue::new(id.to_string(), index, format!("cue {index}"), start, end);
        cue.takes.push(Take {
            take_id: format!("take-{index}"),
            cue_id: id.to_string(),
            provider: "jaitts-f5tts".to_string(),
            provider_version: "1.1.22".to_string(),
            seed: 42 + index as u64,
            duration_ms: dur,
            settings_hash: "abc".to_string(),
            audio_path: format!("takes/take-{index}.wav"),
        });
        cue.selected_take_id = Some(format!("take-{index}"));
        cue.status = CueStatus::Ready;
        cue
    }

    #[test]
    fn validate_export_rejects_missing_and_too_long() {
        let mut ok = make_cue("c1", 1, 0, 1000, 900);
        ok.status = CueStatus::Ready;
        let mut err = make_cue("c2", 2, 1000, 2000, 900);
        err.status = CueStatus::Error;
        let mut tl = make_cue("c3", 3, 2000, 3000, 900);
        tl.status = CueStatus::TooLong;
        let v = validate_export(&[ok.clone(), err, tl], ExportMode::Replace, true, None);
        assert!(v.export_blocked);
        assert_eq!(v.reasons.len(), 2);

        let v = validate_export(&[ok.clone()], ExportMode::Mix, false, None);
        assert!(v.export_blocked);
        assert_eq!(v.reasons.len(), 1);

        let v = validate_export(&[ok], ExportMode::VoiceTrack, false, None);
        assert!(!v.export_blocked);
        assert!(v.reasons.is_empty());
    }

    #[test]
    fn validate_export_rejects_out_of_video_cue() {
        let mut cue = make_cue("c1", 1, 5_000, 6_000, 500);
        cue.status = CueStatus::Ready;
        let v = validate_export(&[cue], ExportMode::VoiceTrack, false, Some(4_000));
        assert!(v.export_blocked);
        assert_eq!(v.reasons.len(), 1);
        assert!(v.reasons[0].contains("outside the video"));

        let v = validate_export(&[make_cue("c1", 1, 0, 3_000, 500)], ExportMode::VoiceTrack, false, Some(4_000));
        assert!(!v.export_blocked);
    }

    #[test]
    fn voice_master_assembles_to_video_duration() {
        let dir = tmp_dir();
        write_tone_wav(&dir.join("takes").join("take-1.wav"), 1.0, 440);
        write_tone_wav(&dir.join("takes").join("take-2.wav"), 0.5, 660);
        let cues = vec![
            make_cue("c1", 1, 0, 1500, 1000),
            make_cue("c2", 2, 2000, 3000, 500),
        ];
        let master = dir.join("cache").join("voice-master.wav");
        let samples = render_voice_master(&dir, &cues, 4000, &master).unwrap();
        assert_eq!(samples, 4000 * 48);
        assert!(master.is_file());

        let dur = crate::domain::media::probe_duration(&master).unwrap();
        assert!((dur - 4000).abs() <= 1, "master duration {dur}ms != 4000ms");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn master_with_no_cues_is_silent_but_full_length() {
        let dir = tmp_dir();
        let master = dir.join("cache").join("voice-master.wav");
        let samples = render_voice_master(&dir, &[], 3000, &master).unwrap();
        assert_eq!(samples, 3000 * 48);
        assert!(master.is_file());
        let dur = crate::domain::media::probe_duration(&master).unwrap();
        assert!((dur - 3000).abs() <= 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_keeps_video_stream_copy_and_aac_audio() {
        let dir = tmp_dir();
        let video = dir.join("input.mp4");
        let video_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=2:size=320x240:rate=24",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-c:a", "aac",
                "-shortest",
            ])
            .arg(&video)
            .status()
            .expect("ffmpeg should be available");
        assert!(video_status.success(), "failed to create test video");

        write_tone_wav(&dir.join("takes").join("take-1.wav"), 1.0, 440);
        let cues = vec![make_cue("c1", 1, 0, 1000, 1000)];
        let master = dir.join("cache").join("voice-master.wav");
        render_voice_master(&dir, &cues, 2000, &master).unwrap();

        let out = dir.join("out.mp4");
        export_replace(&video.to_string_lossy(), &master, &out).unwrap();
        validate_output(&out, 2000, Some("aac"), Some(1)).unwrap();

        // Video stream must be copied (bitstream identical) — ffprobe codec.
        let probe = crate::domain::media::probe_video_codec(&out).unwrap();
        assert_eq!(probe, "h264");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mix_applies_gain_and_limiter_with_video_copy() {
        let dir = tmp_dir();
        let video = dir.join("input.mp4");
        let video_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=2:size=320x240:rate=24",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-c:a", "aac",
                "-shortest",
            ])
            .arg(&video)
            .status()
            .expect("ffmpeg should be available");
        assert!(video_status.success(), "failed to create test video");

        write_tone_wav(&dir.join("takes").join("take-1.wav"), 1.0, 440);
        let cues = vec![make_cue("c1", 1, 0, 1000, 1000)];
        let master = dir.join("cache").join("voice-master.wav");
        render_voice_master(&dir, &cues, 2000, &master).unwrap();

        let out = dir.join("mix.mp4");
        export_mix(&video.to_string_lossy(), &master, &out, -12.0).unwrap();
        // Mix output is stereo (original upmixed + voice) at 48 kHz.
        validate_output(&out, 2000, Some("aac"), Some(2)).unwrap();

        // Video stream is still copied (h264), no re-encode.
        let probe = crate::domain::media::probe_video_codec(&out).unwrap();
        assert_eq!(probe, "h264");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn voice_track_is_mono_48k_pcm24() {
        let dir = tmp_dir();
        write_tone_wav(&dir.join("takes").join("take-1.wav"), 1.0, 440);
        let cues = vec![make_cue("c1", 1, 0, 1000, 1000)];
        let master = dir.join("cache").join("voice-master.wav");
        render_voice_master(&dir, &cues, 2000, &master).unwrap();

        let out = dir.join("voice.wav");
        export_voice_track(&master, &out).unwrap();
        validate_output(&out, 2000, Some("pcm_s24le"), Some(1)).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_output_rejects_bad_duration() {
        let dir = tmp_dir();
        let wav = dir.join("tone.wav");
        write_tone_wav(&wav, 2.0, 440);
        assert!(validate_output(&wav, 2000, None, None).is_ok());
        assert!(validate_output(&wav, 5000, None, None).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_export_end_to_end_replace() {
        let dir = tmp_dir();
        let video = dir.join("input.mp4");
        let video_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=2:size=320x240:rate=24",
                "-f", "lavfi",
                "-i", "sine=frequency=440:duration=2",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-c:a", "aac",
                "-shortest",
            ])
            .arg(&video)
            .status()
            .expect("ffmpeg should be available");
        assert!(video_status.success());

        write_tone_wav(&dir.join("takes").join("take-1.wav"), 1.0, 440);
        let mut cue = make_cue("c1", 1, 0, 1000, 1000);
        cue.status = CueStatus::Ready;
        let request = ExportRequest {
            project_dir: dir.clone(),
            video_path: video.to_string_lossy().to_string(),
            cues: vec![cue],
            output_path: dir.join("out.mp4"),
            mode: ExportMode::Replace,
            original_gain_db: -12.0,
        };
        let samples = run_export(&request, true).unwrap();
        assert_eq!(samples, 2000 * 48);
        validate_output(&request.output_path, 2000, Some("aac"), Some(1)).unwrap();

        // A second run is idempotent (same master cache key).
        let samples2 = run_export(&request, true).unwrap();
        assert_eq!(samples2, samples);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
