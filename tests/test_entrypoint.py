from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

from app.__main__ import application_root


class EntrypointTests(unittest.TestCase):
    def test_source_root_is_repository_root(self) -> None:
        self.assertEqual(application_root(), Path(__file__).resolve().parents[1])

    def test_frozen_root_is_executable_directory(self) -> None:
        with patch.object(sys, "frozen", True, create=True), patch.object(sys, "executable", r"D:\Portable\DubFlow\DubFlow.exe"):
            self.assertEqual(application_root(), Path(r"D:\Portable\DubFlow"))

    def test_runtime_child_uses_installed_application_root(self) -> None:
        with patch.dict(os.environ, {"DUBFLOW_APPLICATION_ROOT": r"D:\DubFlow"}):
            self.assertEqual(application_root(), Path(r"D:\DubFlow"))

if __name__ == "__main__":
    unittest.main()
