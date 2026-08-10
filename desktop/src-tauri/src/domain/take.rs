//! Immutable Take metadata and per-cue selection state.
//!
//! A Take is created once by the TTS provider and must never be mutated
//! after creation. The `Cue` struct holds a list of `Take` instances and
//! the `selected_take_id` pointer.

use serde::{Deserialize, Serialize};

/// Unique identifier for a take (e.g. `"take-001"`).
pub type TakeId = String;

/// Cue status based on generation and timeline solver state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CueStatus {
    #[default]
    #[serde(rename = "Not Generated")]
    NotGenerated,
    #[serde(rename = "Generating")]
    Generating,
    #[serde(rename = "Ready")]
    Ready,
    #[serde(rename = "Adjusted")]
    Adjusted,
    #[serde(rename = "Too Long")]
    TooLong,
    #[serde(rename = "Error")]
    Error,
}

/// An immutable recorded take.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Take {
    pub take_id: TakeId,
    pub cue_id: String,
    pub provider: String,
    pub provider_version: String,
    pub seed: u64,
    pub duration_ms: u64,
    pub settings_hash: String,
    pub audio_path: String,
}

/// A single cue on the subtitle timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    pub id: String,
    pub index: u32,
    pub text: String,
    pub srt_start_ms: i64,
    pub srt_end_ms: i64,
    #[serde(default)]
    pub status: CueStatus,
    #[serde(default)]
    pub selected_take_id: Option<TakeId>,
    #[serde(default)]
    pub takes: Vec<Take>,
    /// True when the cue's takes or selection have changed since the last
    /// project save. Not persisted.
    #[serde(skip)]
    pub dirty: bool,
}

impl Cue {
    pub fn new(id: String, index: u32, text: String, srt_start_ms: i64, srt_end_ms: i64) -> Self {
        Self {
            id,
            index,
            text,
            srt_start_ms,
            srt_end_ms,
            status: CueStatus::NotGenerated,
            selected_take_id: None,
            takes: Vec::new(),
            dirty: false,
        }
    }

    /// Returns the selected take, if any.
    pub fn selected_take(&self) -> Option<&Take> {
        self.selected_take_id
            .as_ref()
            .and_then(|id| self.takes.iter().find(|t| t.take_id == *id))
    }

    /// The duration of the selected take in milliseconds, or 0 if none.
    pub fn selected_duration_ms(&self) -> u64 {
        self.selected_take().map(|t| t.duration_ms).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_take(take_id: &str, duration_ms: u64) -> Take {
        Take {
            take_id: take_id.to_string(),
            cue_id: "cue-001".to_string(),
            provider: "jaitts-f5tts".to_string(),
            provider_version: "1.1.22".to_string(),
            seed: 42,
            duration_ms,
            settings_hash: "abc123".to_string(),
            audio_path: format!("takes/{}.wav", take_id),
        }
    }

    #[test]
    fn new_cue_defaults_to_not_generated() {
        let cue = Cue::new("cue-001".into(), 1, "hello".into(), 1000, 3000);
        assert_eq!(cue.status, CueStatus::NotGenerated);
        assert!(cue.selected_take().is_none());
        assert_eq!(cue.selected_duration_ms(), 0);
    }

    #[test]
    fn selected_take_returns_correct_take() {
        let mut cue = Cue::new("cue-001".into(), 1, "hello".into(), 1000, 3000);
        let t1 = make_take("take-001", 2100);
        let t2 = make_take("take-002", 1900);
        cue.takes = vec![t1, t2];
        cue.selected_take_id = Some("take-002".into());
        assert_eq!(cue.selected_take().unwrap().take_id, "take-002");
        assert_eq!(cue.selected_duration_ms(), 1900);
    }

    #[test]
    fn selected_take_returns_none_for_unknown_id() {
        let mut cue = Cue::new("cue-001".into(), 1, "hello".into(), 1000, 3000);
        cue.takes = vec![make_take("take-001", 2100)];
        cue.selected_take_id = Some("take-999".into());
        assert!(cue.selected_take().is_none());
        assert_eq!(cue.selected_duration_ms(), 0);
    }
}
