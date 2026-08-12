import tempfile
import json
import unittest
from pathlib import Path

from app.settings import SettingsStore


class SettingsTests(unittest.TestCase):
    def test_default_workspace_is_sibling_of_config(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            root = Path(directory)
            settings = SettingsStore(root / "config" / "app-settings.json").load()
            self.assertEqual(Path(settings.workspace_root), (root / "workspace").resolve())

    def test_ignores_settings_removed_by_newer_versions(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            root = Path(directory)
            config = root / "config" / "app-settings.json"
            config.parent.mkdir()
            config.write_text(json.dumps({"workspace_root": str(root / "custom"), "provider_id": "legacy"}), encoding="utf-8")
            settings = SettingsStore(config).load()
            self.assertEqual(Path(settings.workspace_root), root / "custom")


if __name__ == "__main__":
    unittest.main()
