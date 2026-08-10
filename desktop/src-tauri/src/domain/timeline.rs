//! Deterministic Timeline Solver for DubFlow.
//!
//! Given a list of cues with selected take durations, the solver computes
//! render start, render end, speed (1.0–1.25x) and per-cue status.
//!
//! ## Algorithm (per PROJECT_CONTEXT.md)
//!
//! 1. `renderStart = max(SRT start, renderEnd ของ cue ก่อนหน้า)`
//! 2. คำนวณพื้นที่ถึง SRT end และใช้ speed ระหว่าง 1.0–1.25x เท่าที่จำเป็น
//! 3. หากยังยาวเกิน ให้เสียงจบเลย SRT end และส่ง delay ไป cue ถัดไป
//! 4. cue ที่สั้นหรือ gap ภายหลังจะดูดซับ delay; ไม่มีการ slow down ต่ำกว่า 1.0x
//! 5. หากเสียงสุดท้ายจบไม่เกิน video duration ถือว่า recover ได้
//! 6. หากจบเกิน video duration ให้ unresolved ripple chain เป็น `Too Long`

use crate::domain::take::CueStatus;
use serde::{Deserialize, Serialize};

const MAX_SPEED: f64 = 1.25;
const MIN_SPEED: f64 = 1.0;
/// Timeline solver sample rate. All internal calculations use integer samples
/// at 48 kHz to prevent float drift (see PROJECT_CONTEXT.md).
pub const SAMPLE_RATE_HZ: u32 = 48_000;
/// 48 samples per millisecond.
pub const SAMPLES_PER_MS: i64 = 48;

fn ms_to_samples(ms: i64) -> i64 {
    ms * SAMPLES_PER_MS
}

/// Integer ceiling division for non-negative values.
fn ceil_div(a: i64, b: i64) -> i64 {
    debug_assert!(a >= 0 && b > 0);
    (a + b - 1) / b
}

/// Input for the solver — one per cue that has a selected take.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverInput {
    pub cue_id: String,
    pub srt_start_ms: i64,
    pub srt_end_ms: i64,
    pub raw_duration_ms: u64,
}

/// Solver output for one cue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolvedCue {
    pub cue_id: String,
    pub srt_start_sample: i64,
    pub srt_end_sample: i64,
    pub render_start_ms: i64,
    pub render_end_ms: i64,
    pub render_start_sample: i64,
    pub render_end_sample: i64,
    pub speed: f64,
    pub status: CueStatus,
}

/// Result of solving the full timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverResult {
    pub cues: Vec<SolvedCue>,
    pub export_blocked: bool,
    pub total_render_end_ms: i64,
}

impl SolverInput {
    pub fn new(
        cue_id: impl Into<String>,
        srt_start_ms: i64,
        srt_end_ms: i64,
        raw_duration_ms: u64,
    ) -> Self {
        Self {
            cue_id: cue_id.into(),
            srt_start_ms,
            srt_end_ms,
            raw_duration_ms,
        }
    }
}

/// Solve the timeline for a list of cues in SRT order.
///
/// `cues` must be sorted by SRT time. Cues with `raw_duration_ms == 0` are
/// treated as absent (NotGenerated) and do not occupy the timeline.
///
/// `video_duration_ms` is the total video duration. If `None`, the solver
/// skips the final Too-Long check.
pub fn solve(cues: &[SolverInput], video_duration_ms: Option<i64>) -> SolverResult {
    if cues.is_empty() {
        return SolverResult {
            cues: vec![],
            export_blocked: false,
            total_render_end_ms: 0,
        };
    }

    let mut results: Vec<SolvedCue> = Vec::with_capacity(cues.len());
    let mut prev_render_end_sample: i64 = 0;
    // Track the first cue index that started an unresolved overflow chain.
    let mut first_overflow_idx: Option<usize> = None;

    for (i, cue) in cues.iter().enumerate() {
        if cue.raw_duration_ms == 0 {
            // No take selected — does not occupy the timeline.
            // The gap can absorb some delay without affecting this cue.
            let start_sample = ms_to_samples(cue.srt_start_ms);
            results.push(SolvedCue {
                cue_id: cue.cue_id.clone(),
                srt_start_sample: start_sample,
                srt_end_sample: ms_to_samples(cue.srt_end_ms),
                render_start_ms: cue.srt_start_ms,
                render_end_ms: cue.srt_start_ms,
                render_start_sample: start_sample,
                render_end_sample: start_sample,
                speed: 1.0,
                status: CueStatus::NotGenerated,
            });
            continue;
        }

        let raw_dur_sample = ms_to_samples(cue.raw_duration_ms as i64);
        let srt_start_sample = ms_to_samples(cue.srt_start_ms);
        let srt_end_sample = ms_to_samples(cue.srt_end_ms);
        let render_start_sample = std::cmp::max(srt_start_sample, prev_render_end_sample);
        let available_sample = srt_end_sample - render_start_sample;

        let (speed, render_end_sample) = if available_sample <= 0 {
            // The previous cue's render already spilled past this cue's SRT
            // end. There is no time to absorb, so force 1.25x and keep going.
            let render_dur_sample = ceil_div(raw_dur_sample * 4, 5);
            let speed = raw_dur_sample as f64 / render_dur_sample as f64;
            (speed, render_start_sample + render_dur_sample)
        } else if raw_dur_sample <= available_sample {
            // Fits at 1.0x.  No slow-down below 1.0x even if extra space exists.
            (MIN_SPEED, render_start_sample + raw_dur_sample)
        } else {
            let needed_speed = raw_dur_sample as f64 / available_sample as f64;
            if needed_speed <= MAX_SPEED {
                (needed_speed, srt_end_sample)
            } else {
                // Even at 1.25x it overflows — render ends after SRT end.
                let render_dur_sample = ceil_div(raw_dur_sample * 4, 5);
                let speed = raw_dur_sample as f64 / render_dur_sample as f64;
                (speed, render_start_sample + render_dur_sample)
            }
        };

        // Determine status.
        let is_adjusted = render_start_sample != srt_start_sample || speed > MIN_SPEED;
        let status = if is_adjusted {
            CueStatus::Adjusted
        } else {
            CueStatus::Ready
        };

        // Track the overflow chain start.
        if first_overflow_idx.is_none() && render_end_sample > srt_end_sample {
            first_overflow_idx = Some(i);
        }

        // Reset the overflow chain if the gap is large enough to absorb the
        // delay (i.e., render_end <= srt_end_ms after a previous overflow).
        if first_overflow_idx.is_some() && render_end_sample <= srt_end_sample {
            first_overflow_idx = None;
        }

        results.push(SolvedCue {
            cue_id: cue.cue_id.clone(),
            srt_start_sample,
            srt_end_sample,
            render_start_ms: render_start_sample / SAMPLES_PER_MS,
            render_end_ms: render_end_sample / SAMPLES_PER_MS,
            render_start_sample,
            render_end_sample,
            speed,
            status,
        });

        prev_render_end_sample = render_end_sample;
    }

    let total_render_end_sample = prev_render_end_sample;
    let total_render_end_ms = total_render_end_sample / SAMPLES_PER_MS;

    // Check if the unresolved chain exceeds video duration.
    let mut export_blocked = false;
    if let Some(duration) = video_duration_ms {
        if total_render_end_ms > duration {
            export_blocked = true;
            // Mark the overflow chain as Too Long.
            if let Some(start_idx) = first_overflow_idx {
                for solved in results.iter_mut().skip(start_idx) {
                    if solved.status != CueStatus::NotGenerated {
                        solved.status = CueStatus::TooLong;
                    }
                }
            }
        }
    }

    SolverResult {
        cues: results,
        export_blocked,
        total_render_end_ms,
    }
}

/// Check whether any cue in the project is in a state that blocks export.
pub fn export_blocked_by_cue_status(cues: &[crate::domain::take::Cue]) -> bool {
    cues.iter().any(|c| {
        matches!(
            c.status,
            CueStatus::NotGenerated | CueStatus::Error | CueStatus::TooLong
        )
    })
}

/// Check whether a cue falls fully outside the video boundary.
pub fn is_out_of_video(srt_start_ms: i64, _srt_end_ms: i64, video_duration_ms: i64) -> bool {
    srt_start_ms >= video_duration_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(srt_start: i64, srt_end: i64, raw_dur: u64) -> SolverInput {
        SolverInput::new(format!("cue-{:04}", srt_start), srt_start, srt_end, raw_dur)
    }

    // --- Exact fit ---
    #[test]
    fn exact_fit_is_ready() {
        let result = solve(&[c(0, 2000, 2000)], Some(10000));
        assert_eq!(result.cues.len(), 1);
        assert_eq!(result.cues[0].status, CueStatus::Ready);
        assert_eq!(result.cues[0].speed, 1.0);
        assert_eq!(result.cues[0].render_start_ms, 0);
        assert_eq!(result.cues[0].render_end_ms, 2000);
        assert!(!result.export_blocked);
    }

    // --- Short cue (fits with gap) ---
    #[test]
    fn short_cue_is_ready() {
        let result = solve(&[c(1000, 5000, 1500)], Some(10000));
        assert_eq!(result.cues[0].status, CueStatus::Ready);
        assert_eq!(result.cues[0].speed, 1.0);
        assert_eq!(result.cues[0].render_start_ms, 1000);
        assert_eq!(result.cues[0].render_end_ms, 2500);
    }

    // --- 1.25x fit ---
    #[test]
    fn speed_125_fits() {
        // 2000ms available, 2500ms raw → needs 1.25x
        let result = solve(&[c(0, 2000, 2500)], Some(10000));
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert!((result.cues[0].speed - 1.25).abs() < 0.001);
        assert_eq!(result.cues[0].render_end_ms, 2000);
    }

    // --- Speed between 1.0 and 1.25 ---
    #[test]
    fn partial_speed_fits() {
        // 2000ms available, 2200ms raw → needs 1.1x
        let result = solve(&[c(0, 2000, 2200)], Some(10000));
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert!((result.cues[0].speed - 1.1).abs() < 0.001);
        assert_eq!(result.cues[0].render_end_ms, 2000);
    }

    // --- Single cue overflow at 1.25x ---
    #[test]
    fn single_cue_overflow_too_long() {
        // 1000ms available, 2000ms raw → needs 2.0x, capped at 1.25x
        // render_dur = 2000/1.25 = 1600ms, render_end = 0+1600 = 1600ms
        // overflow = 1600 > 1000, final check: 1600 > 10000? No → not blocked
        let result = solve(&[c(0, 1000, 2000)], Some(10000));
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert!((result.cues[0].speed - 1.25).abs() < 0.001);
        assert_eq!(result.cues[0].render_end_ms, 1600);
        assert!(!result.export_blocked);
    }

    // --- Overflow chain that exceeds video duration ---
    #[test]
    fn unresolved_chain_too_long() {
        // Cue 1: 0-1000, raw 2000 → 1.25x, render_end=1600, overflow=600
        // Cue 2: 1500-2000, raw 1000 → render_start=max(1500,1600)=1600, available=400
        //   raw 1000 > 400 → 1.25x, render_dur=800, render_end=2400
        //   overflow=2400 > 2000
        // Final: 2400 > 2000 (video duration) → export blocked, Too Long
        let result = solve(&[c(0, 1000, 2000), c(1500, 2000, 1000)], Some(2000));
        assert!(result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::TooLong);
        assert_eq!(result.cues[1].status, CueStatus::TooLong);
    }

    // --- Ripple absorbed by gap ---
    #[test]
    fn ripple_absorbed_by_gap() {
        // Cue 1: 0-1000, raw 2000 → 1.25x, render_end=1600, overflow=600
        // Cue 2: 5000-8000, raw 1000 → render_start=max(5000,1600)=5000, available=3000
        //   raw 1000 ≤ 3000 → 1.0x, render_end=6000 ≤ 8000 → no overflow
        //   overflow chain reset
        // Final: 6000 ≤ 10000 → OK
        let result = solve(&[c(0, 1000, 2000), c(5000, 8000, 1000)], Some(10000));
        assert!(!result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert_eq!(result.cues[1].status, CueStatus::Ready);
        assert_eq!(result.cues[1].render_start_ms, 5000);
    }

    // --- Multiple cues with ripple, but all recover ---
    #[test]
    fn multi_cue_ripple_recovered() {
        // Cue 1: 0-1000, raw 2000 → 1.25x, render_end=1600, overflow=600
        // Cue 2: 1000-2000, raw 1000 → render_start=max(1000,1600)=1600, available=400
        //   raw 1000 → 1.25x, render_dur=800, render_end=2400, overflow=400
        // Cue 3: 5000-6000, raw 500 → render_start=5000, available=1000
        //   raw 500 ≤ 1000 → 1.0x, render_end=5500, no overflow → chain reset
        // Final: 5500 ≤ 10000 → OK
        let result = solve(
            &[c(0, 1000, 2000), c(1000, 2000, 1000), c(5000, 6000, 500)],
            Some(10000),
        );
        assert!(!result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert_eq!(result.cues[1].status, CueStatus::Adjusted);
        assert_eq!(result.cues[2].status, CueStatus::Ready);
    }

    // --- Unlimited drift inside video bounds ---
    #[test]
    fn unlimited_drift_within_bounds() {
        // A single cue that overflows significantly but still fits within video
        // Cue 1: 0-1000, raw 10000 → 1.25x, render_dur=8000, render_end=8000
        // 8000 ≤ 10000 → OK
        let result = solve(&[c(0, 1000, 10000)], Some(10000));
        assert!(!result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert_eq!(result.cues[0].render_end_ms, 8000);
    }

    // --- Overlapping cue (SRT overlap) handled by solver ---
    #[test]
    fn overlapping_srt_cues() {
        // Cue 1: 0-1000, raw 500 → 1.0x, render_end=500
        // Cue 2: 800-2000, raw 1000 → render_start=max(800,500)=800, available=1200
        //   raw 1000 ≤ 1200 → 1.0x, render_end=1800
        let result = solve(&[c(0, 1000, 500), c(800, 2000, 1000)], Some(10000));
        assert!(!result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::Ready);
        assert_eq!(result.cues[1].status, CueStatus::Ready);
        assert_eq!(result.cues[1].render_start_ms, 800);
        assert_eq!(result.cues[1].render_end_ms, 1800);
    }

    // --- Unrecoverable tail (last render_end > video duration) ---
    #[test]
    fn unrecoverable_tail() {
        // Cue 1: 0-1000, raw 2000 → 1.25x, render_end=1600, overflow=600
        // Final: 1600 > 1500 → export blocked
        let result = solve(&[c(0, 1000, 2000)], Some(1500));
        assert!(result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::TooLong);
    }

    // --- Empty input ---
    #[test]
    fn empty_input() {
        let result = solve(&[], Some(10000));
        assert!(result.cues.is_empty());
        assert!(!result.export_blocked);
    }

    // --- NotGenerated cue (raw_duration_ms == 0) ---
    #[test]
    fn not_generated_cue() {
        let result = solve(&[c(0, 2000, 0)], Some(10000));
        assert_eq!(result.cues[0].status, CueStatus::NotGenerated);
        assert_eq!(result.cues[0].render_start_ms, 0);
        assert_eq!(result.cues[0].render_end_ms, 0);
        assert!(!result.export_blocked);
    }

    // --- NotGenerated cue does not carry ripple ---
    #[test]
    fn not_generated_does_not_carry_ripple() {
        // Cue 1: generated, overflow
        // Cue 2: not generated (gap)
        // Cue 3: generated, should start at its SRT start (gap absorbed)
        let result = solve(
            &[
                c(0, 1000, 2000),    // overflows, render_end=1600
                c(1000, 2000, 0),    // NotGenerated, gap
                c(2000, 5000, 1000), // render_start=max(2000,1600)=2000
            ],
            Some(10000),
        );
        assert!(!result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::Adjusted);
        assert_eq!(result.cues[1].status, CueStatus::NotGenerated);
        assert_eq!(result.cues[2].status, CueStatus::Ready);
        assert_eq!(result.cues[2].render_start_ms, 2000);
    }

    // --- Sample-domain golden: same values as ms but exact at 48k ---
    #[test]
    fn sample_domain_matches_ms_golden() {
        // Cue 1: 0-1000, raw 2000 -> 1.25x, render_end_sample = 0 + ceil(96000*4/5) = 76800
        //   = 1600ms, overflow 600ms
        // Cue 2: 1000-2000, raw 1000 -> render_start_sample = max(48000, 76800) = 76800
        //   available = 96000-76800 = 19200, raw 48000 > 19200 -> 1.25x, render_dur = 38400
        //   render_end_sample = 115200 = 2400ms, overflow 400ms
        // Cue 3: 5000-6000, raw 500 -> render_start_sample = 240000, available 48000
        //   raw 24000 <= 48000 -> 1.0x, render_end_sample = 264000 = 5500ms
        let result = solve(
            &[c(0, 1000, 2000), c(1000, 2000, 1000), c(5000, 6000, 500)],
            Some(10000),
        );
        assert_eq!(result.cues[0].render_start_sample, 0);
        assert_eq!(result.cues[0].render_end_sample, 76_800);
        assert_eq!(result.cues[1].render_start_sample, 76_800);
        assert_eq!(result.cues[1].render_end_sample, 115_200);
        assert_eq!(result.cues[2].render_start_sample, 240_000);
        assert_eq!(result.cues[2].render_end_sample, 264_000);
        assert_eq!(result.total_render_end_ms, 5_500);
    }

    // --- Sample-domain unrecoverable tail ---
    #[test]
    fn sample_domain_tail_exact() {
        // video duration 1500ms = 72000 samples
        // cue 0-1000 raw 2000 -> render_end 76800 > 72000 -> Too Long
        let result = solve(&[c(0, 1000, 2000)], Some(1500));
        assert!(result.export_blocked);
        assert_eq!(result.cues[0].status, CueStatus::TooLong);
        assert_eq!(result.total_render_end_ms, 1600);
    }

    // --- export_blocked_by_cue_status ---
    #[test]
    fn detects_export_blockers() {
        use crate::domain::take::Cue;
        let mut good = Cue::new("g".into(), 1, "ok".into(), 0, 1000);
        good.status = CueStatus::Ready;
        let not_gen = Cue::new("n".into(), 2, "no".into(), 1000, 2000);
        let mut err = Cue::new("e".into(), 3, "err".into(), 2000, 3000);
        err.status = CueStatus::Error;
        let mut too_long = Cue::new("t".into(), 4, "tl".into(), 3000, 4000);
        too_long.status = CueStatus::TooLong;

        assert!(!crate::domain::timeline::export_blocked_by_cue_status(&[
            good.clone()
        ]));
        assert!(crate::domain::timeline::export_blocked_by_cue_status(&[
            not_gen
        ]));
        assert!(crate::domain::timeline::export_blocked_by_cue_status(&[
            err
        ]));
        assert!(crate::domain::timeline::export_blocked_by_cue_status(&[
            too_long
        ]));
    }
}
