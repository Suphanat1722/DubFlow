import unittest

from app.models import Cue, CueStatus
from app.timeline import TimelineSettings, solve_timeline


class TimelineTests(unittest.TestCase):
    def test_speed_fit_preserves_originals(self):
        cue = Cue("cue-0001", 1, 1000, 3000, "hello", generated_duration=2400)
        solve_timeline([cue], TimelineSettings(max_speed=1.25, video_duration_ms=5000))
        self.assertEqual((cue.original_start, cue.original_end), (1000, 3000))
        self.assertAlmostEqual(cue.speed, 1.2)
        self.assertEqual(cue.resolved_end, 3000)
        self.assertEqual(cue.status, CueStatus.ADJUSTED.value)

    def test_ripple_moves_only_unlocked_followers(self):
        first = Cue("cue-0001", 1, 0, 1000, "a", generated_duration=2000)
        second = Cue("cue-0002", 2, 1100, 1800, "b")
        third = Cue("cue-0003", 3, 1900, 2600, "c", lock_timing=True)
        solve_timeline([first, second, third], TimelineSettings(max_speed=1.25))
        self.assertGreater(second.resolved_start, second.original_start)
        self.assertEqual(third.resolved_start, third.original_start)
        self.assertEqual(first.status, CueStatus.NEEDS_REVIEW.value)

    def test_large_gap_stops_ripple(self):
        first = Cue("cue-0001", 1, 0, 1000, "a", generated_duration=5000)
        second = Cue("cue-0002", 2, 2500, 3500, "b")
        solve_timeline([first, second], TimelineSettings(max_speed=1.25, large_gap_ms=1000))
        self.assertEqual(second.resolved_start, 2500)
        self.assertEqual(first.status, CueStatus.NEEDS_REVIEW.value)


if __name__ == "__main__":
    unittest.main()
