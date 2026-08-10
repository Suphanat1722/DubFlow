//! SRT parsing and validation for DubFlow projects.
//!
//! The parser accepts UTF-8 with an optional BOM, multi-line cue text and
//! non-sequential cue indexes. Cue times are measured in milliseconds.

use serde::{Deserialize, Serialize};

/// Maximum supported cue duration. Longer cues are rejected as malformed input.
pub const MAX_CUE_DURATION_MS: i64 = 24 * 60 * 60 * 1000;

/// Maximum supported total media duration. Cues outside this window are
/// reported as `OutOfVideo`.
pub const MAX_VIDEO_DURATION_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SrtCue {
    pub index: u32,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSrt {
    pub cues: Vec<SrtCue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrtErrorKind {
    /// Cue ordering or duration is invalid for the SRT format itself.
    Malformed,
    /// Cue timestamps fall outside the video's media duration.
    OutOfVideo,
    /// Two cues overlap on the timeline.
    Overlap,
    /// The file does not start with a supported SRT cue.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrtError {
    pub kind: SrtErrorKind,
    pub message: String,
}

impl SrtError {
    fn new(kind: SrtErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Parse SRT text and validate cue ordering, overlap and video bounds.
///
/// `video_duration_ms` is the caller-provided media duration. Passing `None`
/// skips out-of-video validation.
pub fn parse_srt(text: &str, video_duration_ms: Option<i64>) -> Result<ParsedSrt, SrtError> {
    let normalized = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = normalized.replace("\r\n", "\n");

    let blocks: Vec<&str> = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .collect();
    if blocks.is_empty() {
        return Err(SrtError::new(
            SrtErrorKind::Empty,
            "SRT contains no cue blocks",
        ));
    }

    let mut cues = Vec::with_capacity(blocks.len());
    for block in blocks {
        cues.push(parse_block(block)?);
    }
    validate_cues(&cues, video_duration_ms)?;
    Ok(ParsedSrt { cues })
}

fn parse_block(block: &str) -> Result<SrtCue, SrtError> {
    let mut lines = block.lines();
    let index_line = lines.next().unwrap_or_default().trim();
    if index_line.is_empty() || !index_line.chars().all(|c| c.is_ascii_digit()) {
        return Err(SrtError::new(
            SrtErrorKind::Malformed,
            format!("cue index is not a non-negative integer: {index_line:?}"),
        ));
    }
    let index = index_line
        .parse::<u32>()
        .map_err(|_| SrtError::new(SrtErrorKind::Malformed, "cue index is too large"))?;

    let timing = lines.next().unwrap_or_default().trim();
    let (start_ms, end_ms) = parse_timing(timing)?;

    let text = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    Ok(SrtCue {
        index,
        start_ms,
        end_ms,
        text,
    })
}

fn parse_timing(timing: &str) -> Result<(i64, i64), SrtError> {
    let err = || {
        SrtError::new(
            SrtErrorKind::Malformed,
            format!("invalid cue timing: {timing:?}"),
        )
    };
    let Some((start, end)) = timing.split_once("-->") else {
        return Err(err());
    };
    let start_ms = parse_timestamp(start.trim()).ok_or_else(err)?;
    let end_ms = parse_timestamp(end.trim()).ok_or_else(err)?;
    Ok((start_ms, end_ms))
}

/// Parse `HH:MM:SS,mmm` or `HH:MM:SS.mmm` into milliseconds.
pub fn parse_timestamp(ts: &str) -> Option<i64> {
    if ts.starts_with('-') {
        return None;
    }
    let (hms, millis) = ts.split_once(',').or_else(|| ts.split_once('.'))?;
    if millis.len() != 3 || !millis.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    let seconds: i64 = parts[2].parse().ok()?;
    if hours < 0 || minutes < 0 || seconds < 0 || minutes > 59 || seconds > 59 {
        return None;
    }
    let ms: i64 = millis.parse().ok()?;
    Some((hours * 3600 + minutes * 60 + seconds) * 1000 + ms)
}

fn validate_cues(cues: &[SrtCue], video_duration_ms: Option<i64>) -> Result<(), SrtError> {
    for cue in cues {
        if cue.end_ms < cue.start_ms {
            return Err(SrtError::new(
                SrtErrorKind::Malformed,
                format!(
                    "cue {} ends before it starts ({} -> {})",
                    cue.index, cue.start_ms, cue.end_ms
                ),
            ));
        }
        if cue.start_ms < 0 {
            return Err(SrtError::new(
                SrtErrorKind::Malformed,
                format!("cue {} has a negative start", cue.index),
            ));
        }
        if cue.end_ms - cue.start_ms > MAX_CUE_DURATION_MS {
            return Err(SrtError::new(
                SrtErrorKind::Malformed,
                format!("cue {} exceeds the maximum duration", cue.index),
            ));
        }
        if let Some(duration) = video_duration_ms {
            if cue.end_ms > duration {
                return Err(SrtError::new(
                    SrtErrorKind::OutOfVideo,
                    format!(
                        "cue {} ends at {}ms but video duration is {}ms",
                        cue.index, cue.end_ms, duration
                    ),
                ));
            }
        }
    }
    for pair in cues.windows(2) {
        if pair[0].end_ms > pair[1].start_ms {
            return Err(SrtError::new(
                SrtErrorKind::Overlap,
                format!(
                    "cue {} overlaps cue {} ({} > {})",
                    pair[0].index, pair[1].index, pair[0].end_ms, pair[1].start_ms
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}1\n00:00:01,000 --> 00:00:03,000\nสวัสดีครับ\n\n2\n00:00:03,500 --> 00:00:05,000\nบรรทัดที่หนึ่ง\nบรรทัดที่สอง\n\n7\n00:00:06,000 --> 00:00:07,250\nGap index";

    #[test]
    fn parses_bom_multiline_and_index_gap() {
        let parsed = parse_srt(SAMPLE, Some(30_000)).unwrap();
        assert_eq!(parsed.cues.len(), 3);
        assert_eq!(parsed.cues[0].index, 1);
        assert_eq!(parsed.cues[0].start_ms, 1_000);
        assert_eq!(parsed.cues[0].end_ms, 3_000);
        assert_eq!(parsed.cues[0].text, "สวัสดีครับ");
        assert_eq!(parsed.cues[1].index, 2);
        assert_eq!(parsed.cues[1].text, "บรรทัดที่หนึ่ง\nบรรทัดที่สอง");
        assert_eq!(parsed.cues[2].index, 7);
        assert_eq!(parsed.cues[2].start_ms, 6_000);
    }

    #[test]
    fn rejects_end_before_start() {
        let err = parse_srt("1\n00:00:03,000 --> 00:00:01,000\nx", None).unwrap_err();
        assert_eq!(err.kind, SrtErrorKind::Malformed);
    }

    #[test]
    fn rejects_negative_start() {
        let err = parse_srt("1\n-00:00:01,000 --> 00:00:02,000\nx", None).unwrap_err();
        assert_eq!(err.kind, SrtErrorKind::Malformed);
    }

    #[test]
    fn rejects_out_of_video() {
        let err = parse_srt("1\n00:00:05,000 --> 00:00:08,000\nx", Some(7_000)).unwrap_err();
        assert_eq!(err.kind, SrtErrorKind::OutOfVideo);
    }

    #[test]
    fn rejects_overlap() {
        let err = parse_srt(
            "1\n00:00:00,000 --> 00:00:03,000\nx\n\n2\n00:00:02,000 --> 00:00:05,000\ny",
            None,
        )
        .unwrap_err();
        assert_eq!(err.kind, SrtErrorKind::Overlap);
    }

    #[test]
    fn parses_dot_timestamp_separator() {
        assert_eq!(parse_timestamp("01:02:03.456"), Some(3_723_456));
        assert_eq!(parse_timestamp("01:02:03,456"), Some(3_723_456));
        assert_eq!(parse_timestamp("99:59:59,999"), Some(359_999_999));
        assert_eq!(parse_timestamp("01:60:00,000"), None);
        assert_eq!(parse_timestamp("01:02:03,45"), None);
    }
}
