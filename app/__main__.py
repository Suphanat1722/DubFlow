from __future__ import annotations

import sys
from multiprocessing import freeze_support
from pathlib import Path


def application_root() -> Path:
    """Return a writable root beside the executable for portable builds."""
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent


def main() -> int:
    freeze_support()
    try:
        from PySide6.QtWidgets import QApplication
    except ImportError:
        print("ยังไม่พบ PySide6: ติดตั้ง dependency ของโปรเจกต์ก่อนเรียก DubFlow", file=sys.stderr)
        return 2

    from app.runtime import activate_runtime
    from app.settings import SettingsStore
    root = application_root()
    settings_store = SettingsStore(root / "config" / "app-settings.json")
    activate_runtime(settings_store.load().runtime_root)

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
