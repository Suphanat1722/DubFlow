import tempfile
import threading
import unittest
import wave
from pathlib import Path

from app.models import Cue
from app.project import ProjectRepository


def write_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(24000)
        output.writeframes(b"\0\0" * 240)


class ProjectTests(unittest.TestCase):
    def test_concurrent_saves_do_not_collide(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            repository = ProjectRepository(Path(directory) / "workspace")
            project, project_dir = repository.create("concurrent")
            errors = []

            def save():
                try:
                    repository.save(project, project_dir)
                except Exception as exc:
                    errors.append(exc)

            threads = [threading.Thread(target=save) for _ in range(8)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join()
            self.assertEqual(errors, [])
            loaded, _ = repository.load(project_dir / "project.json")
            self.assertEqual(loaded.name, "concurrent")

    def test_takes_are_immutable_and_incremented(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            repository = ProjectRepository(Path(directory) / "workspace")
            project, project_dir = repository.create("demo")
            project.cues = [Cue("cue-0001", 1, 0, 1000, "hello")]
            generated = Path(directory) / "generated.wav"
            write_wav(generated)
            first = repository.add_take(project, project_dir, "cue-0001", generated, 900, "fake", "1", 1)
            generated.write_bytes(b"replacement")
            second = repository.add_take(project, project_dir, "cue-0001", generated, 1000, "fake", "1", 2)
            self.assertEqual(first.id, "take-01")
            self.assertEqual(second.id, "take-02")
            self.assertNotEqual((project_dir / first.path).read_bytes(), (project_dir / second.path).read_bytes())
            repository.save(project, project_dir)
            loaded, _ = repository.load(project_dir / "project.json")
            self.assertEqual(len(loaded.cues[0].takes), 2)
            self.assertEqual(loaded.cues[0].selected_take_id, "take-02")
            self.assertFalse(loaded.cues[0].needs_generation)

    def test_raw_take_is_preserved(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            repository = ProjectRepository(Path(directory) / "workspace")
            project, project_dir = repository.create("raw-demo")
            project.cues = [Cue("cue-0001", 1, 0, 1000, "hello")]
            processed = Path(directory) / "processed.wav"
            raw = Path(directory) / "raw.wav"
            write_wav(processed)
            write_wav(raw)
            take = repository.add_take(project, project_dir, "cue-0001", processed, 10, "fake", "1", 1, raw)
            self.assertTrue(take.raw_path.endswith("take-01.raw.wav"))
            self.assertTrue((project_dir / take.raw_path).is_file())

    def test_cue_without_selected_take_needs_generation(self):
        cue = Cue("cue-0001", 1, 0, 1000, "hello")
        self.assertTrue(cue.needs_generation)


if __name__ == "__main__":
    unittest.main()
