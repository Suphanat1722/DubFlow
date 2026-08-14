import math
import shutil
import struct
import tempfile
import unittest
import wave
from pathlib import Path

from app.audio import AudioPipeline, has_active_tail


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

    def test_wav_duration_does_not_require_ffprobe(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            source = Path(directory) / "source.wav"
            with wave.open(str(source), "wb") as audio:
                audio.setparams((1, 2, 24000, 0, "NONE", "not compressed"))
                audio.writeframes(b"\0\0" * 12000)
            pipeline = AudioPipeline(ffprobe="definitely-not-installed")
            self.assertEqual(pipeline.duration_ms(source), 500)

    def test_safe_processing_without_silence_trim_preserves_duration(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            source = Path(directory) / "source.wav"
            output = Path(directory) / "output.wav"
            with wave.open(str(source), "wb") as audio:
                audio.setparams((1, 2, 24000, 0, "NONE", "not compressed"))
                audio.writeframes(b"\0\0" * 2400 + b"\x10\x00" * 24000 + b"\0\0" * 12000)
            pipeline = AudioPipeline()
            pipeline.trim_and_fit(source, output, trim_silence=False)
            self.assertGreaterEqual(pipeline.duration_ms(output), 1590)

    def test_active_model_edge_gets_release_tail(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            source = Path(directory) / "source.wav"
            output = Path(directory) / "output.wav"
            rate = 24000
            frames = [round(10000 * math.sin(2 * math.pi * 220 * sample / rate)) for sample in range(rate)]
            with wave.open(str(source), "wb") as audio:
                audio.setparams((1, 2, rate, 0, "NONE", "not compressed"))
                audio.writeframes(b"".join(struct.pack("<h", frame) for frame in frames))

            self.assertTrue(has_active_tail(source))
            pipeline = AudioPipeline()
            pipeline.trim_and_fit(source, output, trim_silence=False, release_tail=True)

            self.assertGreaterEqual(pipeline.duration_ms(output), 1130)
            with wave.open(str(output), "rb") as audio:
                audio.setpos(audio.getnframes() - audio.getframerate() // 20)
                self.assertEqual(set(audio.readframes(audio.getframerate() // 20)), {0})


if __name__ == "__main__":
    unittest.main()
