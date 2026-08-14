import unittest

from app.audio import assess_transcript


class TranscriptAssessmentTests(unittest.TestCase):
    def test_accepts_noisy_thai_asr_with_matching_ending(self):
        result = assess_transcript(
            "ในคอร์สนี้ คุณจะได้เรียนพื้นฐาน การเขียนโค้ดในยูนิตี้",
            "ใครสนี้คุณจะได้เรียนเพื่อนทานการเห็นข้อดในอยู่นิดตี",
        )
        self.assertTrue(result.complete)
        self.assertGreater(result.coverage, 0.75)

    def test_rejects_transcript_missing_the_ending(self):
        result = assess_transcript(
            "ในคอร์สนี้ คุณจะได้เรียนพื้นฐาน การเขียนโค้ดในยูนิตี้",
            "ใครสนี้คุณจะได้เรียนเพื่อนทานการเห็นข้อดใน",
        )
        self.assertFalse(result.complete)
        self.assertLess(result.suffix_similarity, 0.5)

    def test_rejects_empty_transcript(self):
        result = assess_transcript("สร้างเสียงภาษาไทย", "")
        self.assertFalse(result.complete)
        self.assertEqual(result.coverage, 0.0)


if __name__ == "__main__":
    unittest.main()
