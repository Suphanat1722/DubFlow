import tempfile
import unittest
from pathlib import Path

from app.settings import SettingsStore


class SettingsTests(unittest.TestCase):
    def test_default_workspace_is_sibling_of_config(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            root = Path(directory)
            settings = SettingsStore(root / "config" / "app-settings.json").load()
            self.assertEqual(Path(settings.workspace_root), (root / "workspace").resolve())


if __name__ == "__main__":
    unittest.main()
