from __future__ import annotations

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

if __name__ == "__main__":
    unittest.main()
