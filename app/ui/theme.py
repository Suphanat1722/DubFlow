from pathlib import Path

from PySide6.QtGui import QFont, QFontDatabase


def configure_application_font(application) -> None:
    """Load a Thai-capable system font explicitly, including offscreen renders."""
    for path in (Path("C:/Windows/Fonts/LeelawUI.ttf"), Path("C:/Windows/Fonts/tahoma.ttf")):
        if not path.exists():
            continue
        font_id = QFontDatabase.addApplicationFont(str(path))
        families = QFontDatabase.applicationFontFamilies(font_id)
        if families:
            application.setFont(QFont(families[0], 10))
            return


APP_STYLE = r"""
QMainWindow, QDialog {
    background: #0d111b;
}

QWidget {
    color: #e9edf6;
    font-size: 13px;
}

QFrame#topBar {
    background: #111724;
    border-bottom: 1px solid #242d3e;
}

QFrame#card, QFrame#heroCard, QFrame#stepCard, QFrame#referenceCard {
    background: #151c2a;
    border: 1px solid #283247;
    border-radius: 12px;
}

QFrame#heroCard {
    background: #151d2d;
    border: 1px solid #33415c;
    border-radius: 18px;
}

QFrame#sidebar {
    background: #111724;
    border-right: 1px solid #242d3e;
}

QLabel#brandMark {
    background: #6e73ff;
    color: white;
    border-radius: 8px;
    font-weight: 800;
    font-size: 15px;
    padding: 7px 9px;
}

QLabel#brandName {
    color: #f5f7fb;
    font-size: 17px;
    font-weight: 700;
}

QLabel#eyebrow {
    color: #8f9bb3;
    font-size: 11px;
    font-weight: 700;
}

QLabel#heroTitle {
    color: #ffffff;
    font-size: 30px;
    font-weight: 750;
}

QLabel#pageTitle {
    color: #ffffff;
    font-size: 22px;
    font-weight: 720;
}

QLabel#sectionTitle {
    color: #f3f6fb;
    font-size: 15px;
    font-weight: 700;
}

QLabel#muted, QLabel#helper, QLabel#projectPath {
    color: #98a4b9;
}

QLabel#helper {
    line-height: 1.45;
}

QLabel#statusChip, QLabel#successChip, QLabel#warningChip {
    background: #202a3d;
    color: #c6d0e2;
    border: 1px solid #33405a;
    border-radius: 9px;
    padding: 4px 9px;
    font-size: 11px;
    font-weight: 650;
}

QLabel#successChip {
    background: #14372f;
    color: #82e1bd;
    border-color: #205544;
}

QLabel#warningChip {
    background: #3c2d16;
    color: #f2c66f;
    border-color: #634b24;
}

QPushButton {
    background: #20293a;
    color: #e7ebf3;
    border: 1px solid #344057;
    border-radius: 8px;
    padding: 8px 13px;
    font-weight: 600;
}

QPushButton:hover {
    background: #273249;
    border-color: #465571;
}

QPushButton:pressed {
    background: #1b2231;
    padding-top: 9px;
    padding-bottom: 7px;
}

QPushButton:disabled {
    color: #69758a;
    background: #171d29;
    border-color: #252d3d;
}

QPushButton#primaryButton {
    background: #6e73ff;
    color: white;
    border-color: #7f83ff;
    padding: 9px 16px;
}

QPushButton#primaryButton:hover { background: #7b7fff; }
QPushButton#primaryButton:pressed { background: #5d62e9; }

QPushButton#ghostButton {
    background: transparent;
    border-color: transparent;
    color: #aeb8ca;
}

QPushButton#ghostButton:hover {
    background: #1b2332;
    color: white;
}

QPushButton#stepButton {
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    color: #aab4c6;
    padding: 11px 12px;
}

QPushButton#stepButton:hover {
    background: #192233;
    color: #eef2f8;
}

QPushButton#stepButton[complete="true"] {
    color: #9de5c7;
}

QPushButton#stepButton[current="true"] {
    background: #202849;
    border-color: #414b82;
    color: #ffffff;
}

QPushButton#lockButton:checked {
    background: #342b54;
    color: #c8baff;
    border-color: #66559a;
}

QTableWidget {
    background: #121925;
    alternate-background-color: #151e2d;
    border: 1px solid #283247;
    border-radius: 9px;
    gridline-color: transparent;
    color: #e6ebf4;
    selection-background-color: #303967;
    selection-color: white;
    outline: 0;
}

QTableWidget::item {
    padding: 8px 7px;
    border-bottom: 1px solid #222c3e;
}

QHeaderView::section {
    background: #192131;
    color: #9faabd;
    border: 0;
    border-bottom: 1px solid #303a4f;
    padding: 9px 7px;
    font-size: 11px;
    font-weight: 700;
}

QScrollBar:vertical {
    background: transparent;
    width: 10px;
    margin: 3px;
}
QScrollBar::handle:vertical {
    background: #3a455b;
    border-radius: 4px;
    min-height: 28px;
}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }

QSlider::groove:horizontal {
    height: 4px;
    background: #2b3548;
    border-radius: 2px;
}
QSlider::sub-page:horizontal { background: #7378ff; border-radius: 2px; }
QSlider::handle:horizontal {
    background: #ffffff;
    border: 2px solid #7378ff;
    width: 13px;
    height: 13px;
    margin: -6px 0;
    border-radius: 7px;
}

QProgressBar {
    background: #111824;
    color: #dfe4ef;
    border: 1px solid #303a50;
    border-radius: 6px;
    min-height: 12px;
    max-height: 12px;
    text-align: center;
    font-size: 9px;
}
QProgressBar::chunk {
    background: #6e73ff;
    border-radius: 5px;
}

QLineEdit, QDoubleSpinBox, QComboBox {
    background: #111824;
    color: #f1f4f8;
    border: 1px solid #344057;
    border-radius: 8px;
    padding: 7px 9px;
    selection-background-color: #5960da;
}
QLineEdit:focus, QDoubleSpinBox:focus, QComboBox:focus { border-color: #7378ff; }

QPlainTextEdit, QTextEdit {
    background: #111824;
    color: #f4f7fb;
    border: 1px solid #344057;
    border-radius: 8px;
    padding: 8px;
    selection-background-color: #5960da;
    selection-color: #ffffff;
}
QPlainTextEdit:focus, QTextEdit:focus { border-color: #7378ff; }

QComboBox QAbstractItemView {
    background: #182131;
    color: #eef2f8;
    border: 1px solid #3b4861;
    border-radius: 6px;
    padding: 4px;
    outline: 0;
    selection-background-color: #555dd1;
    selection-color: #ffffff;
}
QComboBox QAbstractItemView::item {
    min-height: 28px;
    padding: 4px 8px;
    color: #eef2f8;
}
QComboBox:disabled {
    background: #171d29;
    color: #737e92;
    border-color: #252d3d;
}

QDialog QLabel, QInputDialog QLabel, QMessageBox QLabel {
    background: transparent;
    color: #edf1f7;
}

QStatusBar {
    background: #0b0f17;
    color: #8995aa;
    border-top: 1px solid #222a39;
}

QSplitter::handle { background: transparent; width: 8px; height: 8px; }
QToolTip {
    background: #242d3e;
    color: white;
    border: 1px solid #46526a;
    padding: 6px;
}
"""
