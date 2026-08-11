import math
import shutil
import struct
import tempfile
import unittest
import wave
from pathlib import Path

from app.audio import AudioPipeline


@unittest.skipUnless(shutil.which("ffmpeg") and shutil.which("ffprobe"), "FFmpeg is not available")
class AudioPipelineTests(unittest.TestCase):
    def test_trim_and_pitch_preserving_fit(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            root = Path(directory)
            source = root / "source.wav"
            output = root / "output.wav"
            rate = 24000
            frames = [0] * (rate // 10)
            frames += [round(8000 * math.sin(2 * math.pi * 440 * sample / rate)) for sample in range(rate)]
            frames += [0] * (rate // 10)
            with wave.open(str(source), "wb") as audio:
                audio.setparams((1, 2, rate, 0, "NONE", "not compressed"))
                audio.writeframes(b"".join(struct.pack("<h", frame) for frame in frames))
            pipeline = AudioPipeline()
            pipeline.trim_and_fit(source, output, 1.2)
            duration = pipeline.duration_ms(output)
            self.assertTrue(output.exists())
            self.assertGreater(duration, 700)
            self.assertLess(duration, 1000)


if __name__ == "__main__":
    unittest.main()
