from __future__ import annotations

import sys
import os
import subprocess
from multiprocessing import freeze_support
from pathlib import Path


def application_root() -> Path:
    """Return a writable root beside the executable for portable builds."""
    override = os.environ.get("DUBFLOW_APPLICATION_ROOT")
    if override:
        return Path(override).resolve()
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent


def launch_external_runtime(root: Path) -> bool:
    """Relaunch the installed source with the selected complete Python runtime."""
    if not getattr(sys, "frozen", False) or os.environ.get("DUBFLOW_RUNTIME_CHILD"):
        return False
    from app.runtime import runtime_python
    from app.settings import SettingsStore

    settings = SettingsStore(root / "config" / "app-settings.json").load()
    python = runtime_python(settings.runtime_root, windowed=True)
    bundle_root = Path(getattr(sys, "_MEIPASS", root))
    source_root = bundle_root / "app_runtime"
    if python is None or not (source_root / "app" / "__main__.py").is_file():
        return False
    environment = os.environ.copy()
    environment["DUBFLOW_APPLICATION_ROOT"] = str(root)
    environment["DUBFLOW_RUNTIME_CHILD"] = "1"
    subprocess.Popen([str(python), "-m", "app"], cwd=source_root, env=environment)
    return True


def main() -> int:
    freeze_support()
    root = application_root()
    if launch_external_runtime(root):
        return 0
    try:
        from PySide6.QtWidgets import QApplication
    except ImportError:
        print("ยังไม่พบ PySide6: ติดตั้ง dependency ของโปรเจกต์ก่อนเรียก DubFlow", file=sys.stderr)
        return 2

    from app.settings import SettingsStore
    settings_store = SettingsStore(root / "config" / "app-settings.json")

    from app.ui.main_window import MainWindow
    from app.ui.theme import configure_application_font

    application = QApplication(sys.argv)
    application.setApplicationName("DubFlow")
    application.setOrganizationName("DubFlow")
    configure_application_font(application)
    window = MainWindow(settings_store)
    window.show()
    return application.exec()


if __name__ == "__main__":
    raise SystemExit(main())
