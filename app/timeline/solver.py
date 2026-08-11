from __future__ import annotations

from dataclasses import dataclass

from app.models import Cue, CueStatus


@dataclass(frozen=True)
class TimelineSettings:
    max_speed: float = 1.25
    large_gap_ms: int = 2000
    video_duration_ms: int | None = None


def solve_timeline(cues: list[Cue], settings: TimelineSettings = TimelineSettings()) -> list[Cue]:
    """Fit cues locally without ever changing the original SRT timing fields.

    Resolution uses the original slot first, then its following free gap. Ripple is
    bounded by a locked cue, a large gap, or the video end.
    """
    for cue in cues:
        cue.warnings = []
        if not cue.lock_timing:
            cue.resolved_start = cue.original_start
            cue.resolved_end = cue.original_end
            cue.timing_shift = 0
        else:
            if cue.resolved_start is None:
                cue.resolved_start = cue.original_start
            if cue.resolved_end is None:
                cue.resolved_end = cue.original_end

    for position, cue in enumerate(cues):
        if cue.lock_timing:
            cue.status = CueStatus.LOCKED.value
            continue
        duration = cue.generated_duration
        if duration is None:
            cue.final_duration = None
            cue.speed = 1.0
            if cue.timing_shift:
                cue.warnings = [warning for warning in cue.warnings if not warning.startswith("Shift ")]
                cue.warnings.append(f"Shift +{cue.timing_shift}ms")
            continue

        slot = max(1, cue.slot_duration)
        start = cue.resolved_start if cue.resolved_start is not None else cue.original_start
        required_speed = duration / slot
        speed = max(1.0, min(settings.max_speed, required_speed))
        final_duration = round(duration / speed)
        resolved_end = start + final_duration

        cue.resolved_start = start
        cue.resolved_end = resolved_end
        cue.final_duration = final_duration
        cue.speed = speed
        cue.timing_shift = start - cue.original_start

        if speed > 1.001:
            cue.warnings.append(f"Speed {speed:.2f}x")
        original_end_at_shift = cue.original_end + cue.timing_shift
        if resolved_end > original_end_at_shift:
            cue.warnings.append(f"Duration overflow +{resolved_end - original_end_at_shift}ms")

        unresolved = False
        previous_end = resolved_end
        for following_position in range(position + 1, len(cues)):
            following = cues[following_position]
            following_start = following.resolved_start if following.resolved_start is not None else following.original_start
            overlap = previous_end - following_start
            if overlap <= 0:
                break
            original_gap = following.original_start - cues[following_position - 1].original_end
            if following.lock_timing or original_gap >= settings.large_gap_ms:
                unresolved = True
                break
            following.resolved_start = following_start + overlap
            following.resolved_end = (following.resolved_end if following.resolved_end is not None else following.original_end) + overlap
            following.timing_shift = following.resolved_start - following.original_start
            following.warnings = [warning for warning in following.warnings if not warning.startswith("Shift ")]
            following.warnings.append(f"Shift +{following.timing_shift}ms")
            if following.status not in (CueStatus.NOT_GENERATED.value, CueStatus.LOCKED.value):
                following.status = CueStatus.ADJUSTED.value
            previous_end = following.resolved_end

        if settings.video_duration_ms is not None and resolved_end > settings.video_duration_ms:
            unresolved = True
        if unresolved:
            cue.status = CueStatus.NEEDS_REVIEW.value
            cue.warnings.append("Ripple ถูกหยุดที่ขอบเขต")
        elif speed > 1.001 or resolved_end > original_end_at_shift or cue.timing_shift:
            cue.status = CueStatus.ADJUSTED.value
        else:
            cue.status = CueStatus.READY.value
    return cues
