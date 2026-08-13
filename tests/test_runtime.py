import tempfile
import unittest
from pathlib import Path

from app.runtime import runtime_python, runtime_site_packages


class RuntimePathTests(unittest.TestCase):
    def test_resolves_virtual_environment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Lib" / "site-packages" / "torch").mkdir(parents=True)
            self.assertEqual(runtime_site_packages(root), (root / "Lib" / "site-packages").resolve())

    def test_rejects_environment_without_torch(self):
        with tempfile.TemporaryDirectory() as temporary:
            self.assertIsNone(runtime_site_packages(temporary))

    def test_resolves_runtime_interpreter(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Lib" / "site-packages" / "torch").mkdir(parents=True)
            python = root / "Scripts" / "python.exe"
            python.parent.mkdir()
            python.touch()
            self.assertEqual(runtime_python(root), python.resolve())


if __name__ == "__main__":
    unittest.main()
