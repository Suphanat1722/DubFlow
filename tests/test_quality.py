import unittest

from app.audio import assess_take_quality


class TakeQualityTests(unittest.TestCase):
    def test_flags_suspiciously_short_mixed_language_take(self):
        warnings = assess_take_quality("เช่น Audio Source หรือ Collider", 2460, 817, 817)
        self.assertTrue(any("อาจพูดไม่ครบ" in warning for warning in warnings))

    def test_flags_aggressive_postprocessing_loss(self):
        warnings = assess_take_quality("ประโยคทดสอบตามปกติ", 2500, 3000, 1200)
        self.assertTrue(any("การตัด silence" in warning for warning in warnings))

    def test_accepts_normal_take(self):
        self.assertEqual(assess_take_quality("ประโยคทดสอบ", 2000, 1800, 1700), [])


if __name__ == "__main__":
    unittest.main()
