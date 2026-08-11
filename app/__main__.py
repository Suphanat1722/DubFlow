from __future__ import annotations

import sys
from pathlib import Path


def main() -> int:
    try:
        from PySide6.QtWidgets import QApplication
    except ImportError:
        print("ยังไม่พบ PySide6: ติดตั้ง dependency ของโปรเจกต์ก่อนเรียก DubFlow", file=sys.stderr)
        return 2

    from app.settings import SettingsStore
    from app.ui.main_window import MainWindow
    from app.ui.theme import configure_application_font

    application = QApplication(sys.argv)
    application.setApplicationName("DubFlow")
    application.setOrganizationName("DubFlow")
    configure_application_font(application)
    root = Path(__file__).resolve().parent.parent
    window = MainWindow(SettingsStore(root / "config" / "app-settings.json"))
    window.show()
    return application.exec()


if __name__ == "__main__":
    raise SystemExit(main())
