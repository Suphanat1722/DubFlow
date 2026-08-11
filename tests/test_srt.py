import tempfile
import unittest
from pathlib import Path

from app.subtitles import SrtError, parse_srt, parse_srt_file


class SrtTests(unittest.TestCase):
    def test_multiline_and_original_timing(self):
        cues = parse_srt("1\n00:00:01,250 --> 00:00:03,500\nสวัสดี\nโลก\n")
        self.assertEqual(len(cues), 1)
        self.assertEqual(cues[0].original_start, 1250)
        self.assertEqual(cues[0].original_end, 3500)
        self.assertEqual(cues[0].text, "สวัสดี\nโลก")

    def test_cp874_file(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            path = Path(directory) / "thai.srt"
            path.write_bytes("1\n00:00:00,000 --> 00:00:01,000\nทดสอบ\n".encode("cp874"))
            self.assertEqual(parse_srt_file(path)[0].text, "ทดสอบ")

    def test_rejects_reverse_time(self):
        with self.assertRaises(SrtError):
            parse_srt("1\n00:00:02,000 --> 00:00:01,000\nผิด\n")


if __name__ == "__main__":
    unittest.main()
