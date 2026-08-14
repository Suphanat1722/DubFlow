import unittest

from app.tts.jaitts import _fixed_total_duration


class JaiTtsDurationTests(unittest.TestCase):
    def test_compensates_mixed_thai_english_underallocation(self):
        reference = "สวัสดีครับ วันนี้อากาศค่อนข้างดี ผมกำลังทดสอบระบบสร้างเสียงพากย์ภาษาไทย เพื่อให้เสียงฟังเป็นธรรมชาติและชัดเจนมากที่สุด. "

        fixed = _fixed_total_duration(
            10.0,
            reference,
            "เช่น Audio Source หรือ Collider",
            2460,
        )

        self.assertIsNotNone(fixed)
        self.assertGreater(fixed, 12.5)

    def test_leaves_thai_only_text_on_upstream_estimate(self):
        fixed = _fixed_total_duration(8.0, "เสียงอ้างอิงภาษาไทย", "สร้างเสียงภาษาไทย", 2500)
        self.assertIsNone(fixed)

    def test_leaves_balanced_mixed_text_on_upstream_estimate(self):
        fixed = _fixed_total_duration(4.0, "ทดสอบ Audio Source", "ทดสอบ Audio Source", 4000)
        self.assertIsNone(fixed)


if __name__ == "__main__":
    unittest.main()
