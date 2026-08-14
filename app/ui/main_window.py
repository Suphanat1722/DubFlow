from __future__ import annotations

import random
import time
import uuid
from pathlib import Path

from PySide6.QtCore import QThreadPool, QUrl, Qt
from PySide6.QtGui import QAction, QColor, QKeySequence
from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer
from PySide6.QtMultimediaWidgets import QVideoWidget
from PySide6.QtWidgets import (
    QAbstractItemView,
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFileDialog,
    QFrame,
    QFormLayout,
    QGridLayout,
    QHBoxLayout,
    QHeaderView,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QProgressBar,
    QSlider,
    QSplitter,
    QStackedWidget,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from app.audio import AudioPipeline, ExportMode, TranscriptAssessment, TranscriptVerifier, assess_take_quality, has_active_tail
from app.models import Cue, CueStatus, Project, ReferenceVoice
from app.project import ProjectRepository
from app.runtime import RuntimeManager
from app.settings import AppSettings, SettingsStore
from app.subtitles import parse_srt_file
from app.timeline import TimelineSettings, solve_timeline
from app.tts import GenerationRequest, JaiTTSProvider
from app.video import probe_media

from .timeline_widget import TimelineWidget
from .theme import APP_STYLE
from .workers import BackgroundTask


def _clock(milliseconds: int | None) -> str:
    if milliseconds is None:
        return "—"
    seconds, millis = divmod(milliseconds, 1000)
    minutes, seconds = divmod(seconds, 60)
    hours, minutes = divmod(minutes, 60)
    return f"{hours:02d}:{minutes:02d}:{seconds:02d}.{millis:03d}"


def _transcript_warning(assessment: TranscriptAssessment) -> str:
    heard = assessment.transcript.replace("\n", " ").strip()
    if len(heard) > 90:
        heard = heard[:87] + "…"
    return (
        f"ASR ตรวจพบว่าอาจพูดไม่ครบ (เนื้อหา {assessment.coverage:.0%} · คำท้าย {assessment.suffix_similarity:.0%})"
        + (f" · ได้ยิน: {heard}" if heard else "")
    )


class SettingsDialog(QDialog):
    def __init__(self, settings: AppSettings, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Settings")
        self.workspace = QLineEdit(settings.workspace_root)
        browse = QPushButton("เลือก…")
        browse.clicked.connect(self._browse)
        workspace_row = QHBoxLayout()
        workspace_row.addWidget(self.workspace)
        workspace_row.addWidget(browse)
        self.runtime = QLineEdit(settings.runtime_root)
        self.runtime.setPlaceholderText(r"โฟลเดอร์ .venv ที่มี PyTorch เช่น E:\DubFlow\.venv")
        runtime_browse = QPushButton("เลือก…")
        runtime_browse.clicked.connect(self._browse_runtime)
        runtime_row = QHBoxLayout()
        runtime_row.addWidget(self.runtime)
        runtime_row.addWidget(runtime_browse)
        self.asr_model = QLineEdit(settings.asr_model_root)
        self.asr_model.setPlaceholderText(r"ค่าเริ่มต้น: Workspace\models\asr\whisper-base")
        asr_browse = QPushButton("เลือก…")
        asr_browse.clicked.connect(self._browse_asr_model)
        asr_row = QHBoxLayout()
        asr_row.addWidget(self.asr_model)
        asr_row.addWidget(asr_browse)
        self.max_speed = QDoubleSpinBox()
        self.max_speed.setRange(1.0, 2.0)
        self.max_speed.setSingleStep(0.05)
        self.max_speed.setValue(settings.max_speed)
        form = QFormLayout(self)
        form.addRow("Workspace", workspace_row)
        form.addRow("AI Runtime", runtime_row)
        runtime_help = QLabel("ไม่ใช่โฟลเดอร์โมเดล · เลือก Python .venv ที่ติดตั้ง torch, torchaudio และ f5-tts แล้ว\nโมเดลจะดาวน์โหลดอัตโนมัติไปที่ Workspace\\models เมื่อสร้างเสียงครั้งแรก")
        runtime_help.setObjectName("helper")
        runtime_help.setWordWrap(True)
        form.addRow("", runtime_help)
        form.addRow("โมเดลตรวจคำพูด", asr_row)
        asr_help = QLabel("เลือกโฟลเดอร์ Whisper ที่มี model.safetensors · หากเว้นว่างจะค้นหาที่ Workspace\\models\\asr\\whisper-base")
        asr_help.setObjectName("helper")
        asr_help.setWordWrap(True)
        form.addRow("", asr_help)
        form.addRow("ความเร็วสูงสุด", self.max_speed)
        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        form.addRow(buttons)

    def _browse(self) -> None:
        path = QFileDialog.getExistingDirectory(self, "เลือก Workspace", self.workspace.text())
        if path:
            self.workspace.setText(path)

    def _browse_runtime(self) -> None:
        path = QFileDialog.getExistingDirectory(self, "เลือก Python Runtime (.venv)", self.runtime.text())
        if path:
            self.runtime.setText(path)

    def _browse_asr_model(self) -> None:
        path = QFileDialog.getExistingDirectory(self, "เลือกโมเดล Whisper", self.asr_model.text() or self.workspace.text())
        if path:
            self.asr_model.setText(path)


class ExportDialog(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Export")
        self.mode = QComboBox()
        self.mode.addItem("Voice Track Only", ExportMode.VOICE_ONLY)
        self.mode.addItem("Replace Original Audio", ExportMode.REPLACE_AUDIO)
        self.mode.addItem("Mix Voice + Original", ExportMode.MIX)
        self.voice = QDoubleSpinBox()
        self.voice.setRange(0, 2)
        self.voice.setValue(1)
        self.original = QDoubleSpinBox()
        self.original.setRange(0, 2)
        self.original.setValue(0.35)
        self.ducking = QCheckBox("ลดเสียงต้นฉบับขณะมีเสียงพากย์")
        self.ducking.setChecked(True)
        form = QFormLayout(self)
        form.addRow("รูปแบบ", self.mode)
        form.addRow("เสียงพากย์", self.voice)
        form.addRow("เสียงต้นฉบับ", self.original)
        form.addRow(self.ducking)
        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        form.addRow(buttons)


class MainWindow(QMainWindow):
    columns = ["#", "ช่วงเวลา", "ข้อความพากย์", "Take", "ความยาว", "การปรับ", "สถานะ"]

    def __init__(self, settings_store: SettingsStore):
        super().__init__()
        self.settings_store = settings_store
        self.settings = settings_store.load()
        self.repository = ProjectRepository(self.settings.workspace_root)
        self.audio_pipeline = AudioPipeline(self.settings.ffmpeg_path, self.settings.ffprobe_path)
        self.provider = JaiTTSProvider()
        self.provider_loaded = False
        self.transcript_verifier = TranscriptVerifier()
        self.project: Project | None = None
        self.project_dir: Path | None = None
        self.video_duration = 0
        self._updating_table = False
        self._busy = False
        self.active_task: BackgroundTask | None = None
        self._generation_started_at = 0.0
        self._last_completion_message = ""

        self.thread_pool = QThreadPool(self)
        self.thread_pool.setMaxThreadCount(1)
        self.media_audio = QAudioOutput(self)
        self.media_player = QMediaPlayer(self)
        self.media_player.setAudioOutput(self.media_audio)
        self.take_audio = QAudioOutput(self)
        self.take_player = QMediaPlayer(self)
        self.take_player.setAudioOutput(self.take_audio)

        self.setWindowTitle("DubFlow — AI SRT Voice Generator")
        self.resize(1480, 920)
        self.setMinimumSize(1120, 720)
        self.setStyleSheet(APP_STYLE)
        self._build_ui()
        self._connect_media()
        self._show_runtime()
        self._refresh()

    def _build_ui(self) -> None:
        self.busy_actions: list[QWidget | QAction] = []
        self.pages = QStackedWidget()
        self.welcome_page = self._build_welcome_page()
        self.editor_page = self._build_editor_page()
        self.pages.addWidget(self.welcome_page)
        self.pages.addWidget(self.editor_page)
        self.setCentralWidget(self.pages)
        self._install_shortcuts()
        self.statusBar().showMessage("พร้อม")

    def _brand(self) -> QWidget:
        widget = QWidget()
        layout = QHBoxLayout(widget)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(9)
        mark = QLabel("DF")
        mark.setObjectName("brandMark")
        mark.setAlignment(Qt.AlignCenter)
        name = QLabel("DubFlow")
        name.setObjectName("brandName")
        layout.addWidget(mark)
        layout.addWidget(name)
        return widget

    def _button(self, text: str, callback, primary: bool = False, ghost: bool = False) -> QPushButton:
        button = QPushButton(text)
        if primary:
            button.setObjectName("primaryButton")
        elif ghost:
            button.setObjectName("ghostButton")
        button.setCursor(Qt.PointingHandCursor)
        button.clicked.connect(callback)
        self.busy_actions.append(button)
        return button

    def _build_welcome_page(self) -> QWidget:
        page = QWidget()
        outer = QVBoxLayout(page)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        top = QFrame()
        top.setObjectName("topBar")
        top_layout = QHBoxLayout(top)
        top_layout.setContentsMargins(28, 14, 28, 14)
        top_layout.addWidget(self._brand())
        top_layout.addStretch()
        settings = self._button("ตั้งค่า", self.open_settings, ghost=True)
        top_layout.addWidget(settings)
        outer.addWidget(top)

        body = QWidget()
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(52, 42, 52, 42)
        body_layout.setSpacing(22)
        body_layout.addStretch()

        hero = QFrame()
        hero.setObjectName("heroCard")
        hero.setMaximumWidth(1080)
        hero_layout = QGridLayout(hero)
        hero_layout.setContentsMargins(44, 38, 44, 38)
        hero_layout.setHorizontalSpacing(54)
        hero_layout.setVerticalSpacing(14)

        eyebrow = QLabel("AI SRT VOICE STUDIO")
        eyebrow.setObjectName("eyebrow")
        title = QLabel("สร้างเสียงพากย์จากซับไตเติล\nโดยไม่ต้องจัดเวลาเอง")
        title.setObjectName("heroTitle")
        title.setWordWrap(True)
        description = QLabel("นำเข้าวิดีโอและ SRT เลือกเสียงอ้างอิง แล้วให้ DubFlow สร้างเสียง ปรับความยาว และวางลง Timeline อัตโนมัติ")
        description.setObjectName("helper")
        description.setWordWrap(True)
        description.setMaximumWidth(610)
        actions = QHBoxLayout()
        new_button = self._button("สร้างโปรเจกต์ใหม่", self.create_project, primary=True)
        new_button.setMinimumHeight(42)
        open_button = self._button("เปิดโปรเจกต์เดิม", self.open_project)
        open_button.setMinimumHeight(42)
        actions.addWidget(new_button)
        actions.addWidget(open_button)
        actions.addStretch()

        hero_layout.addWidget(eyebrow, 0, 0)
        hero_layout.addWidget(title, 1, 0)
        hero_layout.addWidget(description, 2, 0)
        hero_layout.addLayout(actions, 3, 0)

        guide = QFrame()
        guide.setObjectName("stepCard")
        guide_layout = QVBoxLayout(guide)
        guide_layout.setContentsMargins(24, 22, 24, 22)
        guide_layout.setSpacing(13)
        guide_title = QLabel("เริ่มง่ายใน 5 ขั้นตอน")
        guide_title.setObjectName("sectionTitle")
        guide_layout.addWidget(guide_title)
        for number, text in enumerate(("นำเข้าวิดีโอ", "นำเข้าไฟล์ SRT", "เลือกเสียงอ้างอิง", "สร้างและตรวจเสียง", "Export ไฟล์พร้อมใช้"), 1):
            row = QHBoxLayout()
            badge = QLabel(str(number))
            badge.setFixedSize(25, 25)
            badge.setAlignment(Qt.AlignCenter)
            badge.setStyleSheet("background:#2b3550;color:#bfc8ff;border-radius:12px;font-weight:700")
            label = QLabel(text)
            row.addWidget(badge)
            row.addWidget(label)
            row.addStretch()
            guide_layout.addLayout(row)
        guide_layout.addStretch()
        self.welcome_runtime_label = QLabel("กำลังตรวจสอบ GPU…")
        self.welcome_runtime_label.setObjectName("successChip")
        self.welcome_runtime_label.setWordWrap(True)
        guide_layout.addWidget(self.welcome_runtime_label)
        hero_layout.addWidget(guide, 0, 1, 4, 1)
        hero_layout.setColumnStretch(0, 3)
        hero_layout.setColumnStretch(1, 2)

        centered = QHBoxLayout()
        centered.addStretch()
        centered.addWidget(hero)
        centered.addStretch()
        body_layout.addLayout(centered)
        hint = QLabel("ไฟล์ต้นฉบับจะไม่ถูกแก้ไข และทุกครั้งที่สร้างเสียงจะได้ Take ใหม่เสมอ")
        hint.setObjectName("muted")
        hint.setAlignment(Qt.AlignCenter)
        body_layout.addWidget(hint)
        body_layout.addStretch()
        outer.addWidget(body, 1)
        return page

    def _build_editor_page(self) -> QWidget:
        page = QWidget()
        outer = QVBoxLayout(page)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        top = QFrame()
        top.setObjectName("topBar")
        top_layout = QHBoxLayout(top)
        top_layout.setContentsMargins(22, 11, 22, 11)
        top_layout.setSpacing(8)
        top_layout.addWidget(self._brand())
        divider = QFrame()
        divider.setFixedWidth(1)
        divider.setStyleSheet("background:#2a3345")
        top_layout.addWidget(divider)
        self.project_path_label = QLabel("")
        self.project_path_label.setObjectName("projectPath")
        top_layout.addWidget(self.project_path_label)
        top_layout.addStretch()
        top_layout.addWidget(self._button("โปรเจกต์ใหม่", self.create_project, ghost=True))
        top_layout.addWidget(self._button("เปิด", self.open_project, ghost=True))
        top_layout.addWidget(self._button("ตั้งค่า", self.open_settings, ghost=True))
        outer.addWidget(top)

        content = QHBoxLayout()
        content.setContentsMargins(0, 0, 0, 0)
        content.setSpacing(0)
        sidebar = self._build_sidebar()
        content.addWidget(sidebar)

        workspace = QWidget()
        workspace_layout = QVBoxLayout(workspace)
        workspace_layout.setContentsMargins(24, 20, 24, 18)
        workspace_layout.setSpacing(14)
        heading = QHBoxLayout()
        heading_text = QVBoxLayout()
        heading_text.setSpacing(2)
        self.project_title = QLabel("โปรเจกต์")
        self.project_title.setObjectName("pageTitle")
        self.project_summary = QLabel("เตรียมไฟล์ให้ครบ แล้วเริ่มสร้างเสียง")
        self.project_summary.setObjectName("muted")
        heading_text.addWidget(self.project_title)
        heading_text.addWidget(self.project_summary)
        heading.addLayout(heading_text)
        heading.addStretch()
        self.auto_fit_button = self._button("จัด Timeline ใหม่", self.auto_fit)
        self.export_button = self._button("Export", self.export_project)
        self.generate_all_button = self._button("สร้างเสียงทั้งหมด", self.generate_all, primary=True)
        heading.addWidget(self.auto_fit_button)
        heading.addWidget(self.export_button)
        heading.addWidget(self.generate_all_button)
        workspace_layout.addLayout(heading)

        self.task_panel = QFrame()
        self.task_panel.setObjectName("card")
        task_layout = QHBoxLayout(self.task_panel)
        task_layout.setContentsMargins(13, 9, 13, 9)
        self.task_label = QLabel("กำลังเตรียมงาน…")
        self.task_label.setMinimumWidth(260)
        self.task_progress = QProgressBar()
        self.task_progress.setRange(0, 0)
        self.task_progress.setTextVisible(False)
        self.cancel_task_button = QPushButton("หยุดหลังรายการนี้")
        self.cancel_task_button.clicked.connect(self.cancel_active_task)
        task_layout.addWidget(self.task_label)
        task_layout.addWidget(self.task_progress, 1)
        task_layout.addWidget(self.cancel_task_button)
        self.task_panel.hide()
        workspace_layout.addWidget(self.task_panel)

        splitter = QSplitter(Qt.Horizontal)
        splitter.setChildrenCollapsible(False)
        splitter.addWidget(self._build_preview_panel())
        splitter.addWidget(self._build_subtitle_panel())
        splitter.setStretchFactor(0, 2)
        splitter.setStretchFactor(1, 3)
        splitter.setSizes([440, 760])
        workspace_layout.addWidget(splitter, 1)

        timeline_card = QFrame()
        timeline_card.setObjectName("card")
        timeline_layout = QVBoxLayout(timeline_card)
        timeline_layout.setContentsMargins(14, 10, 14, 10)
        timeline_header = QHBoxLayout()
        timeline_title = QLabel("Timeline overview")
        timeline_title.setObjectName("sectionTitle")
        self.timeline_hint = QLabel("SRT ด้านบน · เสียงพากย์ด้านล่าง")
        self.timeline_hint.setObjectName("muted")
        timeline_header.addWidget(timeline_title)
        timeline_header.addStretch()
        timeline_header.addWidget(self.timeline_hint)
        timeline_layout.addLayout(timeline_header)
        self.timeline = TimelineWidget()
        self.timeline.setMinimumHeight(74)
        timeline_layout.addWidget(self.timeline)
        workspace_layout.addWidget(timeline_card)
        content.addWidget(workspace, 1)

        content_host = QWidget()
        content_host.setLayout(content)
        outer.addWidget(content_host, 1)
        return page

    def _build_sidebar(self) -> QWidget:
        sidebar = QFrame()
        sidebar.setObjectName("sidebar")
        sidebar.setFixedWidth(252)
        layout = QVBoxLayout(sidebar)
        layout.setContentsMargins(18, 22, 18, 18)
        layout.setSpacing(6)
        label = QLabel("ขั้นตอนของโปรเจกต์")
        label.setObjectName("eyebrow")
        layout.addWidget(label)
        self.progress_label = QLabel("0 จาก 5 ขั้นตอน")
        self.progress_label.setObjectName("muted")
        layout.addWidget(self.progress_label)
        layout.addSpacing(8)
        steps = [
            ("1   วิดีโอต้นฉบับ", self.import_video),
            ("2   ซับไตเติล SRT", self.import_srt),
            ("3   เสียงอ้างอิง", self.select_reference),
            ("4   สร้างและตรวจเสียง", self.generate_all),
            ("5   Export", self.export_project),
        ]
        self.step_buttons: list[QPushButton] = []
        for text, callback in steps:
            button = self._button(text, callback)
            button.setObjectName("stepButton")
            button.setProperty("complete", False)
            button.setProperty("current", False)
            layout.addWidget(button)
            self.step_buttons.append(button)
        layout.addStretch()
        privacy = QLabel("ประมวลผลบนเครื่อง\nไฟล์ต้นฉบับไม่ถูกเขียนทับ")
        privacy.setObjectName("muted")
        privacy.setWordWrap(True)
        layout.addWidget(privacy)
        self.gpu_chip = QLabel("กำลังตรวจ GPU…")
        self.gpu_chip.setObjectName("successChip")
        self.gpu_chip.setWordWrap(True)
        layout.addWidget(self.gpu_chip)
        return sidebar

    def _build_preview_panel(self) -> QWidget:
        panel = QFrame()
        panel.setObjectName("card")
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(14, 14, 14, 14)
        layout.setSpacing(10)
        header = QHBoxLayout()
        title = QLabel("ตัวอย่างวิดีโอ")
        title.setObjectName("sectionTitle")
        self.video_name_label = QLabel("ยังไม่ได้เลือกวิดีโอ")
        self.video_name_label.setObjectName("muted")
        header.addWidget(title)
        header.addStretch()
        header.addWidget(self.video_name_label)
        layout.addLayout(header)

        self.preview_stack = QStackedWidget()
        self.video_empty = QFrame()
        empty_layout = QVBoxLayout(self.video_empty)
        empty_layout.addStretch()
        empty_title = QLabel("เริ่มด้วยวิดีโอต้นฉบับ")
        empty_title.setObjectName("sectionTitle")
        empty_title.setAlignment(Qt.AlignCenter)
        empty_help = QLabel("รองรับ MP4, MOV, MKV, AVI และ WebM")
        empty_help.setObjectName("muted")
        empty_help.setAlignment(Qt.AlignCenter)
        choose_video = self._button("เลือกไฟล์วิดีโอ", self.import_video, primary=True)
        choose_video.setMaximumWidth(180)
        empty_layout.addWidget(empty_title)
        empty_layout.addWidget(empty_help)
        choose_row = QHBoxLayout()
        choose_row.addStretch()
        choose_row.addWidget(choose_video)
        choose_row.addStretch()
        empty_layout.addLayout(choose_row)
        empty_layout.addStretch()
        self.preview_stack.addWidget(self.video_empty)

        video_page = QWidget()
        video_layout = QVBoxLayout(video_page)
        video_layout.setContentsMargins(0, 0, 0, 0)
        video_layout.setSpacing(0)
        self.video = QVideoWidget()
        self.video.setMinimumSize(390, 235)
        self.video.setStyleSheet("background:#05070b;border-radius:8px")
        self.media_player.setVideoOutput(self.video)
        video_layout.addWidget(self.video, 1)
        self.preview_stack.addWidget(video_page)
        self.preview_stack.setMinimumHeight(260)
        layout.addWidget(self.preview_stack, 1)

        self.subtitle_preview = QLabel("ยังไม่มี Subtitle")
        self.subtitle_preview.setAlignment(Qt.AlignCenter)
        self.subtitle_preview.setWordWrap(True)
        self.subtitle_preview.setMinimumHeight(52)
        self.subtitle_preview.setStyleSheet("font-size:16px;font-weight:600;padding:10px;background:#101722;border:1px solid #263044;border-radius:8px")
        layout.addWidget(self.subtitle_preview)
        controls = QHBoxLayout()
        self.play_button = QPushButton("เล่น")
        self.play_button.setFixedWidth(62)
        self.play_button.clicked.connect(self.toggle_video)
        self.seek = QSlider(Qt.Horizontal)
        self.seek.sliderMoved.connect(self.media_player.setPosition)
        self.time_label = QLabel("00:00 / 00:00")
        self.time_label.setObjectName("muted")
        controls.addWidget(self.play_button)
        controls.addWidget(self.seek, 1)
        controls.addWidget(self.time_label)
        layout.addLayout(controls)

        reference = QFrame()
        reference.setObjectName("referenceCard")
        reference_layout = QHBoxLayout(reference)
        reference_layout.setContentsMargins(13, 11, 13, 11)
        reference_text = QVBoxLayout()
        reference_title = QLabel("เสียงอ้างอิง")
        reference_title.setObjectName("sectionTitle")
        self.reference_label = QLabel("ยังไม่ได้เลือก · ใช้เสียงพูดชัดเจน 3–12 วินาที")
        self.reference_label.setObjectName("muted")
        self.reference_label.setWordWrap(True)
        reference_text.addWidget(reference_title)
        reference_text.addWidget(self.reference_label)
        reference_layout.addLayout(reference_text, 1)
        reference_layout.addWidget(self._button("เลือกเสียง", self.select_reference))
        layout.addWidget(reference)
        return panel

    def _build_subtitle_panel(self) -> QWidget:
        panel = QFrame()
        panel.setObjectName("card")
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(14, 14, 14, 14)
        layout.setSpacing(10)
        header = QHBoxLayout()
        header_text = QVBoxLayout()
        title = QLabel("รายการเสียงพากย์")
        title.setObjectName("sectionTitle")
        self.subtitle_count_label = QLabel("นำเข้า SRT เพื่อเริ่มต้น")
        self.subtitle_count_label.setObjectName("muted")
        header_text.addWidget(title)
        header_text.addWidget(self.subtitle_count_label)
        header.addLayout(header_text)
        header.addStretch()
        header.addWidget(self._button("นำเข้า SRT", self.import_srt))
        layout.addLayout(header)

        self.subtitle_stack = QStackedWidget()
        empty = QWidget()
        empty_layout = QVBoxLayout(empty)
        empty_layout.addStretch()
        empty_title = QLabel("ยังไม่มีซับไตเติล")
        empty_title.setObjectName("sectionTitle")
        empty_title.setAlignment(Qt.AlignCenter)
        empty_help = QLabel("ไฟล์ SRT จะกลายเป็นรายการเสียงพากย์ที่แก้ข้อความและสร้างใหม่ได้ทีละบรรทัด")
        empty_help.setObjectName("muted")
        empty_help.setWordWrap(True)
        empty_help.setAlignment(Qt.AlignCenter)
        import_button = self._button("เลือกไฟล์ SRT", self.import_srt, primary=True)
        import_button.setMaximumWidth(170)
        empty_layout.addWidget(empty_title)
        empty_layout.addWidget(empty_help)
        import_row = QHBoxLayout()
        import_row.addStretch()
        import_row.addWidget(import_button)
        import_row.addStretch()
        empty_layout.addLayout(import_row)
        empty_layout.addStretch()
        self.subtitle_stack.addWidget(empty)

        self.table = QTableWidget(0, len(self.columns))
        self.table.setHorizontalHeaderLabels(self.columns)
        self.table.setSelectionBehavior(QAbstractItemView.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SingleSelection)
        self.table.setAlternatingRowColors(True)
        self.table.setShowGrid(False)
        self.table.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.table.verticalHeader().setVisible(False)
        self.table.verticalHeader().setDefaultSectionSize(48)
        self.table.itemChanged.connect(self._table_changed)
        self.table.doubleClicked.connect(lambda _index: self.play_selected_take())
        self.table.itemSelectionChanged.connect(self._selection_changed)
        header_view = self.table.horizontalHeader()
        header_view.setSectionResizeMode(QHeaderView.Fixed)
        header_view.setSectionResizeMode(2, QHeaderView.Stretch)
        for column, width in enumerate((36, 108, 220, 72, 64, 66, 68)):
            self.table.setColumnWidth(column, width)
        self.subtitle_stack.addWidget(self.table)
        layout.addWidget(self.subtitle_stack, 1)

        row_actions = QHBoxLayout()
        row_actions.addWidget(self._button("ฟัง Take", self.play_selected_take))
        row_actions.addWidget(self._button("สร้างใหม่", self.generate_selected))
        row_actions.addWidget(self._button("เลือก Take", self.choose_take))
        self.lock_take_button = self._button("ล็อก Take", self.toggle_take_lock)
        self.lock_take_button.setObjectName("lockButton")
        self.lock_take_button.setCheckable(True)
        self.lock_timing_button = self._button("ล็อกเวลา", self.toggle_timing_lock)
        self.lock_timing_button.setObjectName("lockButton")
        self.lock_timing_button.setCheckable(True)
        row_actions.addWidget(self.lock_take_button)
        row_actions.addWidget(self.lock_timing_button)
        row_actions.addStretch()
        layout.addLayout(row_actions)
        return panel

    def _install_shortcuts(self) -> None:
        for shortcut, callback in (("Ctrl+N", self.create_project), ("Ctrl+O", self.open_project), ("Ctrl+S", self.save_project), ("Ctrl+G", self.generate_all)):
            action = QAction(self)
            action.setShortcut(QKeySequence(shortcut))
            action.triggered.connect(callback)
            self.addAction(action)

    def _connect_media(self) -> None:
        self.media_player.positionChanged.connect(self._position_changed)
        self.media_player.durationChanged.connect(self._duration_changed)
        self.media_player.playbackStateChanged.connect(lambda state: self.play_button.setText("พัก" if state == QMediaPlayer.PlayingState else "เล่น"))
        self.media_player.errorOccurred.connect(lambda _error, message: self._error(f"เล่นวิดีโอไม่ได้: {message}"))

    def _show_runtime(self) -> None:
        task = BackgroundTask(RuntimeManager(self.settings.runtime_root).detect)
        self.runtime_task = task
        task.signals.result.connect(self._apply_runtime)
        task.signals.error.connect(lambda message: self.statusBar().showMessage(f"ตรวจ GPU ไม่สำเร็จ: {message}"))
        self.thread_pool.start(task)

    def _apply_runtime(self, info) -> None:
        message = f"{info.gpu_name or 'CPU'} · {info.vram_mb // 1024 if info.vram_mb else 0} GB · {'CUDA พร้อม' if info.cuda_available else info.mode}"
        self.gpu_chip.setText(message)
        self.welcome_runtime_label.setText(message)
        chip_name = "successChip" if info.cuda_available else "warningChip"
        self.gpu_chip.setObjectName(chip_name)
        self.welcome_runtime_label.setObjectName(chip_name)
        self.gpu_chip.style().unpolish(self.gpu_chip)
        self.gpu_chip.style().polish(self.gpu_chip)
        self.welcome_runtime_label.style().unpolish(self.welcome_runtime_label)
        self.welcome_runtime_label.style().polish(self.welcome_runtime_label)
        self.statusBar().showMessage(f"{info.mode} · {message}")

    def _require_project(self) -> bool:
        if self.project is None or self.project_dir is None:
            QMessageBox.information(self, "DubFlow", "กรุณาสร้างหรือเปิดโปรเจกต์ก่อน")
            return False
        return True

    def create_project(self) -> None:
        name, ok = QInputDialog.getText(self, "New Project", "ชื่อโปรเจกต์")
        if not ok or not name.strip():
            return
        try:
            self.repository = ProjectRepository(self.settings.workspace_root)
            self.project, self.project_dir = self.repository.create(name.strip())
            self.video_duration = 0
            self.media_player.setSource(QUrl())
            self._refresh()
        except Exception as exc:
            self._error(str(exc))

    def open_project(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, "Open Project", str(Path(self.settings.workspace_root) / "projects"), "DubFlow Project (project.json)")
        if not path:
            return
        try:
            self.project, self.project_dir = self.repository.load(path)
            self.video_duration = 0
            if self.project.video_path and Path(self.project.video_path).exists():
                self._load_video(self.project.video_path)
            else:
                self.media_player.setSource(QUrl())
            self._refresh()
        except Exception as exc:
            self._error(str(exc))

    def save_project(self) -> None:
        if self._require_project():
            self.repository.save(self.project, self.project_dir)
            self.statusBar().showMessage("บันทึกโปรเจกต์แล้ว", 4000)

    def import_video(self) -> None:
        if not self._require_project():
            return
        path, _ = QFileDialog.getOpenFileName(self, "Import Video", "", "Video (*.mp4 *.mov *.mkv *.avi *.webm);;All files (*)")
        if path:
            self.project.video_path = str(Path(path).resolve())
            self._load_video(path)
            self.save_project()
            self._refresh()

    def _load_video(self, path: str) -> None:
        self.media_player.setSource(QUrl.fromLocalFile(path))
        try:
            self.video_duration = probe_media(path, self.settings.ffprobe_path).duration_ms
        except Exception:
            self.video_duration = 0
        self.timeline.set_data(self.project.cues if self.project else [], self.video_duration)

    def import_srt(self) -> None:
        if not self._require_project():
            return
        path, _ = QFileDialog.getOpenFileName(self, "Import SRT", "", "SubRip (*.srt)")
        if not path:
            return
        try:
            self.project.cues = parse_srt_file(path)
            self.project.srt_path = str(Path(path).resolve())
            self.save_project()
            self._refresh()
        except Exception as exc:
            self._error(str(exc))

    def select_reference(self) -> None:
        if not self._require_project():
            return
        choices = ["ไฟล์เสียงจากภายนอก", "ดึงเสียงจากวิดีโอ"]
        choice, ok = QInputDialog.getItem(self, "ตั้งค่าเสียงอ้างอิง", "เลือกแหล่งเสียง", choices, editable=False)
        if not ok:
            return
        start = end = None
        if choice == choices[0]:
            source, _ = QFileDialog.getOpenFileName(self, "Reference Audio", "", "Audio (*.wav *.mp3 *.m4a *.flac *.ogg);;All files (*)")
        else:
            if not self.project.video_path:
                self._error("กรุณา Import Video ก่อน")
                return
            source = self.project.video_path
            start_seconds, ok = QInputDialog.getDouble(self, "ช่วงอ้างอิง", "เริ่มที่ (วินาที)", 0, 0, 999999, 3)
            if not ok:
                return
            duration, ok = QInputDialog.getDouble(self, "ช่วงอ้างอิง", "ความยาว 3–12 วินาที", 5, 3, 12, 2)
            if not ok:
                return
            start, end = round(start_seconds * 1000), round((start_seconds + duration) * 1000)
        if not source:
            return
        transcript, ok = QInputDialog.getMultiLineText(
            self,
            "ข้อความในเสียงอ้างอิง",
            "พิมพ์ข้อความที่พูดในไฟล์เสียงให้ตรงทุกคำ\nข้อความนี้ช่วยให้ AI เรียนรู้น้ำเสียงได้ถูกต้อง",
        )
        if not ok or not transcript.strip():
            return
        output = self.project_dir / "cache" / "reference.wav"

        def operation():
            self.audio_pipeline.prepare_reference(source, output, start, end)
            return ReferenceVoice("video" if start is not None else "external", str(Path(source).resolve()), output.relative_to(self.project_dir).as_posix(), transcript.strip(), start, end)

        def finished(reference):
            self.project.reference = reference
            self.save_project()
            self._refresh()

        self._run_task("กำลังเตรียม Reference…", operation, finished)

    def _selected_cue(self) -> Cue | None:
        row = self.table.currentRow()
        if self.project and 0 <= row < len(self.project.cues):
            return self.project.cues[row]
        return None

    def _validate_generation(self) -> bool:
        if not self._require_project():
            return False
        reference = self.project.reference
        if not reference.processed_path or not (self.project_dir / reference.processed_path).exists():
            self._error("กรุณาเลือกและเตรียม Reference Voice ก่อน")
            return False
        return True

    def _generate_cues(self, cues: list[Cue], progress=None, should_cancel=None):
        if not self.provider_loaded:
            self.provider.load(Path(self.settings.workspace_root) / "models", Path(self.settings.workspace_root) / "cache" / "models")
            self.provider_loaded = True
        asr_model_dir = (
            Path(self.settings.asr_model_root)
            if self.settings.asr_model_root
            else Path(self.settings.workspace_root) / "models" / "asr" / "whisper-base"
        )
        transcript_verification_enabled = self.transcript_verifier.load(asr_model_dir)
        reference = self.project.reference
        reference_path, reference_text = self.provider.prepare_reference(self.project_dir / reference.processed_path, reference.transcript)
        completed = 0
        attempted = 0
        failures: list[dict[str, object]] = []
        consecutive_failures = 0
        total = len(cues)
        for cue in cues:
            if should_cancel and should_cancel():
                break
            cue.status = CueStatus.GENERATING.value
            if progress:
                progress(completed, total, cue.id)
            artifacts: list[Path] = []
            try:
                candidates: list[dict[str, object]] = []
                max_quality_attempts = 3 if transcript_verification_enabled else 2
                for quality_attempt in range(max_quality_attempts):
                    temporary = self.project_dir / "cache" / f"generation-{cue.id}-{uuid.uuid4().hex}.wav"
                    processed = self.project_dir / "cache" / f"processed-{cue.id}-{uuid.uuid4().hex}.wav"
                    artifacts.extend((temporary, processed))
                    request = GenerationRequest(
                        cue.text,
                        reference_path,
                        reference_text,
                        temporary,
                        random.randint(1, 2_147_483_647),
                        target_duration_ms=cue.slot_duration,
                    )
                    try:
                        result = self.provider.generate(request)
                        raw_duration_ms = self.audio_pipeline.duration_ms(result.path)
                        active_tail = has_active_tail(result.path)
                        assessment = None
                        if transcript_verification_enabled:
                            try:
                                assessment = self.transcript_verifier.verify(cue.text, result.path)
                            except Exception:
                                # Transcript verification is an optional quality
                                # layer and must never discard a valid TTS Take.
                                transcript_verification_enabled = False
                        self.audio_pipeline.trim_and_fit(result.path, processed, release_tail=active_tail)
                        duration_ms = self.audio_pipeline.duration_ms(processed)
                        quality_warnings = assess_take_quality(cue.text, cue.slot_duration, raw_duration_ms, duration_ms)
                        if assessment is not None and not assessment.complete:
                            quality_warnings.append(_transcript_warning(assessment))
                        if any(warning.startswith("การตัด silence") for warning in quality_warnings):
                            # Prefer a little extra silence over losing a soft
                            # word ending. Reprocess the immutable raw output.
                            self.audio_pipeline.trim_and_fit(
                                result.path,
                                processed,
                                trim_silence=False,
                                release_tail=active_tail,
                            )
                            duration_ms = self.audio_pipeline.duration_ms(processed)
                            quality_warnings = assess_take_quality(cue.text, cue.slot_duration, raw_duration_ms, duration_ms)
                            if assessment is not None and not assessment.complete:
                                quality_warnings.append(_transcript_warning(assessment))
                        candidates.append({
                            "raw": result.path,
                            "processed": processed,
                            "duration": duration_ms,
                            "seed": result.seed,
                            "warnings": quality_warnings,
                            "asr_complete": assessment.complete if assessment is not None else True,
                            "asr_coverage": assessment.coverage if assessment is not None else 0.0,
                        })
                        if not quality_warnings:
                            break
                    except Exception:
                        if not candidates:
                            raise
                        break

                best = min(
                    candidates,
                    key=lambda item: (
                        not bool(item["asr_complete"]),
                        len(item["warnings"]),
                        -float(item["asr_coverage"]),
                        -int(item["duration"]),
                    ),
                )
                self.repository.add_take(
                    self.project,
                    self.project_dir,
                    cue.id,
                    best["processed"],
                    int(best["duration"]),
                    self.provider.id,
                    self.provider.version,
                    int(best["seed"]),
                    best["raw"],
                )
                cue.warnings = [
                    warning for warning in cue.warnings
                    if not warning.startswith(("สร้างเสียงไม่สำเร็จ:", "เสียงสั้นผิดปกติ", "การตัด silence", "ASR ตรวจพบ"))
                ]
                cue.warnings.extend(best["warnings"])
                cue.status = CueStatus.NEEDS_REVIEW.value if best["warnings"] else CueStatus.READY.value
                self.repository.save(self.project, self.project_dir)
                completed += 1
                attempted += 1
                consecutive_failures = 0
                if progress:
                    progress(attempted, total, cue.id)
            except Exception as exc:
                cue.status = CueStatus.ERROR.value
                message = str(exc) or exc.__class__.__name__
                cue.warnings = [warning for warning in cue.warnings if not warning.startswith("สร้างเสียงไม่สำเร็จ:")]
                cue.warnings.append(f"สร้างเสียงไม่สำเร็จ: {message}")
                failures.append({"index": cue.index, "message": message})
                attempted += 1
                consecutive_failures += 1
                self.repository.save(self.project, self.project_dir)
                if progress:
                    progress(attempted, total, cue.id)
                # A broken sentence/file should not stop the batch. Repeated
                # failures usually mean the shared runtime/model is broken, so
                # stop before hundreds of identical retries.
                if consecutive_failures >= 3:
                    break
            finally:
                for artifact in artifacts:
                    artifact.unlink(missing_ok=True)
        self.repository.save(self.project, self.project_dir)
        return {
            "completed": completed,
            "failed": failures,
            "total": total,
            "cancelled": bool(should_cancel and should_cancel()),
            "stopped_after_failures": consecutive_failures >= 3,
            "processed": attempted,
        }

    def generate_selected(self) -> None:
        cue = self._selected_cue()
        if cue is None:
            self._error("กรุณาเลือก Subtitle")
            return
        if self._validate_generation():
            self._run_generation([cue])

    def generate_all(self) -> None:
        if not self._validate_generation():
            return
        cues = [cue for cue in self.project.cues if not cue.lock_take and cue.needs_generation]
        if not cues:
            QMessageBox.information(self, "DubFlow", "ทุก Subtitle มี Take แล้ว ไม่มีรายการที่ต้องสร้างต่อ")
            return
        self._run_generation(cues)

    def _after_generation(self, result=None) -> None:
        self.auto_fit()
        self._refresh()
        if not result:
            return
        failed = result.get("failed", [])
        if result.get("cancelled"):
            self._last_completion_message = f"หยุดแล้ว · สำเร็จ {result['completed']} · ผิดพลาด {len(failed)} · กดสร้างทั้งหมดเพื่อทำต่อ"
        elif result.get("stopped_after_failures"):
            self._last_completion_message = f"หยุดหลังผิดพลาดติดกัน 3 รายการ · สำเร็จ {result['completed']} · กดสร้างทั้งหมดเพื่อลองเฉพาะรายการที่ยังไม่สำเร็จ"
        elif failed:
            indexes = ", ".join(str(item["index"]) for item in failed[:8])
            suffix = "…" if len(failed) > 8 else ""
            self._last_completion_message = f"ทำต่อจนจบ · สำเร็จ {result['completed']} · ผิดพลาด {len(failed)} (รายการ {indexes}{suffix}) · กดสร้างทั้งหมดเพื่อลองรายการที่เสียอีกครั้ง"
        else:
            self._last_completion_message = f"สร้างเสียงสำเร็จ {result['completed']} รายการ"

    def auto_fit(self) -> None:
        if not self._require_project():
            return
        solve_timeline(self.project.cues, TimelineSettings(self.settings.max_speed, self.settings.large_gap_ms, self.video_duration or None))
        self.save_project()
        self._refresh()

    def choose_take(self) -> None:
        cue = self._selected_cue()
        if cue is None or not cue.takes:
            self._error("Subtitle นี้ยังไม่มี Take")
            return
        labels = [f"{take.id} · {_clock(take.duration_ms)} · seed {take.seed}" for take in cue.takes]
        selected, ok = QInputDialog.getItem(self, "Select Take", "Take", labels, editable=False)
        if ok:
            cue.selected_take_id = cue.takes[labels.index(selected)].id
            self.save_project()
            self._refresh()

    def toggle_take_lock(self) -> None:
        cue = self._selected_cue()
        if cue is None:
            self.lock_take_button.setChecked(False)
            return
        cue.lock_take = self.lock_take_button.isChecked()
        self.save_project()
        self._refresh()

    def toggle_timing_lock(self) -> None:
        cue = self._selected_cue()
        if cue is None:
            self.lock_timing_button.setChecked(False)
            return
        cue.lock_timing = self.lock_timing_button.isChecked()
        self.save_project()
        self._refresh()

    def play_selected_take(self) -> None:
        cue = self._selected_cue()
        if cue is None or cue.selected_take is None or self.project_dir is None:
            return
        self.take_player.setSource(QUrl.fromLocalFile(str(self.project_dir / cue.selected_take.path)))
        self.take_player.play()

    def export_project(self) -> None:
        if not self._require_project():
            return
        dialog = ExportDialog(self)
        if dialog.exec() != QDialog.Accepted:
            return
        mode = dialog.mode.currentData()
        suffix = ".wav" if mode == ExportMode.VOICE_ONLY else ".mp4"
        output, _ = QFileDialog.getSaveFileName(self, "Export", str(self.project_dir / "export" / f"{self.project.name}{suffix}"), f"Media (*{suffix})")
        if output:
            self._run_task("กำลัง Export…", lambda: self.audio_pipeline.export(self.project, self.project_dir, output, mode, dialog.voice.value(), dialog.original.value(), dialog.ducking.isChecked()), lambda path: QMessageBox.information(self, "Export", f"Export สำเร็จ\n{path}"))

    def open_settings(self) -> None:
        dialog = SettingsDialog(self.settings, self)
        if dialog.exec() == QDialog.Accepted:
            self.settings.workspace_root = str(Path(dialog.workspace.text()).resolve())
            runtime_root = dialog.runtime.text().strip()
            self.settings.runtime_root = str(Path(runtime_root).resolve()) if runtime_root else ""
            asr_model_root = dialog.asr_model.text().strip()
            self.settings.asr_model_root = str(Path(asr_model_root).resolve()) if asr_model_root else ""
            self.settings.max_speed = dialog.max_speed.value()
            self.settings_store.save(self.settings)
            self.repository = ProjectRepository(self.settings.workspace_root)
            self._show_runtime()
            self.statusBar().showMessage("บันทึก Settings แล้ว · หากเปลี่ยน AI Runtime ให้เปิด DubFlow ใหม่", 8000)

    def toggle_video(self) -> None:
        if self.media_player.playbackState() == QMediaPlayer.PlayingState:
            self.media_player.pause()
        else:
            self.media_player.play()

    def _duration_changed(self, duration: int) -> None:
        self.seek.setRange(0, duration)
        if duration:
            self.video_duration = duration
            self.timeline.set_data(self.project.cues if self.project else [], duration)

    def _position_changed(self, position: int) -> None:
        self.seek.blockSignals(True)
        self.seek.setValue(position)
        self.seek.blockSignals(False)
        self.timeline.set_position(position)
        self.time_label.setText(f"{_clock(position)[3:8]} / {_clock(self.media_player.duration())[3:8]}")
        current = next((cue for cue in self.project.cues if cue.original_start <= position < cue.original_end), None) if self.project else None
        self.subtitle_preview.setText(current.text if current else "")

    def _refresh(self) -> None:
        if self.project is None or self.project_dir is None:
            self.pages.setCurrentWidget(self.welcome_page)
            self.setWindowTitle("DubFlow — AI SRT Voice Generator")
            return

        self.pages.setCurrentWidget(self.editor_page)
        self.setWindowTitle(f"DubFlow — {self.project.name}")
        self.project_title.setText(self.project.name)
        self.project_path_label.setText(str(self.project_dir))

        has_video = bool(self.project.video_path and Path(self.project.video_path).exists())
        has_cues = bool(self.project.cues)
        reference_path = self.project.reference.processed_path
        has_reference = bool(reference_path and (self.project_dir / reference_path).exists())
        has_generated = bool(has_cues and all(cue.selected_take is not None for cue in self.project.cues))
        has_export = any((self.project_dir / "export").glob("*"))
        completed = [has_video, has_cues, has_reference, has_generated, has_export]
        step_names = ["วิดีโอต้นฉบับ", "ซับไตเติล SRT", "เสียงอ้างอิง", "สร้างและตรวจเสียง", "Export"]
        current_step = next((index for index, done in enumerate(completed) if not done), 4)
        for index, (button, done, name) in enumerate(zip(self.step_buttons, completed, step_names)):
            button.setText(f"{index + 1}   {name}")
            button.setProperty("complete", done)
            button.setProperty("current", index == current_step and not done)
            button.style().unpolish(button)
            button.style().polish(button)
        self.progress_label.setText(f"{sum(completed)} จาก 5 ขั้นตอน")

        generated_count = sum(cue.selected_take is not None for cue in self.project.cues)
        review_count = sum(cue.status == CueStatus.NEEDS_REVIEW.value for cue in self.project.cues)
        if has_cues:
            summary = f"{len(self.project.cues)} บรรทัด · สร้างแล้ว {generated_count}"
            if review_count:
                summary += f" · ต้องตรวจ {review_count}"
        else:
            summary = "นำเข้าวิดีโอและ SRT เพื่อเริ่มสร้างเสียงพากย์"
        self.project_summary.setText(summary)
        self.generate_all_button.setText("สร้างเสียงที่เหลือ" if generated_count else "สร้างเสียงทั้งหมด")

        self.preview_stack.setCurrentIndex(1 if has_video else 0)
        self.video_name_label.setText(Path(self.project.video_path).name if has_video else "ยังไม่ได้เลือกวิดีโอ")
        if not has_video:
            self.subtitle_preview.setText("ยังไม่ได้เลือกวิดีโอ")
        elif has_cues and not self.subtitle_preview.text().strip():
            self.subtitle_preview.setText("กดเล่นเพื่อดู Subtitle ตามเวลา")
        self.subtitle_stack.setCurrentIndex(1 if has_cues else 0)
        self.subtitle_count_label.setText(f"{len(self.project.cues)} บรรทัด · ดับเบิลคลิกเพื่อฟัง Take" if has_cues else "นำเข้า SRT เพื่อเริ่มต้น")

        self.generate_all_button.setEnabled(has_cues and has_reference and not self._busy)
        self.auto_fit_button.setEnabled(has_cues and not self._busy)
        self.export_button.setEnabled(generated_count > 0 and not self._busy)

        self._updating_table = True
        cues = self.project.cues
        selected_row = self.table.currentRow()
        self.table.setRowCount(len(cues))
        status_labels = {
            CueStatus.NOT_GENERATED.value: "ยังไม่สร้าง",
            CueStatus.GENERATING.value: "กำลังสร้าง",
            CueStatus.READY.value: "พร้อม",
            CueStatus.ADJUSTED.value: "ปรับแล้ว",
            CueStatus.NEEDS_REVIEW.value: "ต้องตรวจ",
            CueStatus.ERROR.value: "ผิดพลาด",
            CueStatus.LOCKED.value: "ล็อก",
        }
        for row, cue in enumerate(cues):
            take_text = cue.selected_take_id or "—"
            if len(cue.takes) > 1:
                take_text += f" · {len(cue.takes)} takes"
            adjustments = []
            if cue.speed > 1.001:
                adjustments.append(f"{cue.speed:.2f}x")
            if cue.timing_shift:
                adjustments.append(f"{cue.timing_shift:+d}ms")
            values = [
                str(cue.index),
                f"{_clock(cue.original_start)[3:8]} – {_clock(cue.original_end)[3:8]}",
                cue.text,
                take_text,
                f"{cue.generated_duration / 1000:.2f}s" if cue.generated_duration is not None else "—",
                " · ".join(adjustments) or "พอดี",
                status_labels.get(cue.status, cue.status),
            ]
            for column, value in enumerate(values):
                item = QTableWidgetItem(value)
                if column != 2:
                    item.setFlags(item.flags() & ~Qt.ItemIsEditable)
                item.setToolTip("\n".join(cue.warnings) if cue.warnings else cue.text)
                if cue.status == CueStatus.NEEDS_REVIEW.value:
                    item.setForeground(QColor("#f2bd6b"))
                elif cue.status == CueStatus.READY.value:
                    if column == 6:
                        item.setForeground(QColor("#78d9b3"))
                elif cue.status == CueStatus.ERROR.value:
                    item.setForeground(QColor("#f27d86"))
                self.table.setItem(row, column, item)
        self._updating_table = False
        if cues:
            self.table.selectRow(selected_row if 0 <= selected_row < len(cues) else 0)
        self._selection_changed()
        self.timeline.set_data(cues, self.video_duration)
        reference = self.project.reference
        if has_reference:
            self.reference_label.setText(f"{Path(reference.original_path).name} · พร้อมใช้งาน\n{reference.transcript}")
        else:
            self.reference_label.setText("ยังไม่ได้เลือก · ใช้เสียงพูดชัดเจน 3–12 วินาที")

    def _table_changed(self, item: QTableWidgetItem) -> None:
        if self._updating_table or not self.project or item.row() >= len(self.project.cues):
            return
        # Programmatic status/Take updates must not trigger a second project
        # save from the UI while the generation worker is saving the same file.
        if item.column() != 2:
            return
        cue = self.project.cues[item.row()]
        cue.text = item.text().strip()
        self.repository.save(self.project, self.project_dir)

    def _selection_changed(self) -> None:
        cue = self._selected_cue()
        self.lock_take_button.blockSignals(True)
        self.lock_timing_button.blockSignals(True)
        self.lock_take_button.setChecked(bool(cue and cue.lock_take))
        self.lock_timing_button.setChecked(bool(cue and cue.lock_timing))
        self.lock_take_button.setEnabled(cue is not None and not self._busy)
        self.lock_timing_button.setEnabled(cue is not None and not self._busy)
        self.lock_take_button.blockSignals(False)
        self.lock_timing_button.blockSignals(False)

    def _run_task(self, message: str, operation, on_result) -> None:
        task = BackgroundTask(operation)
        self._start_task(task, message, on_result, cancellable=False)

    def _run_generation(self, cues: list[Cue]) -> None:
        if self._busy:
            return
        self._generation_started_at = time.monotonic()
        task = BackgroundTask(lambda: None)
        task.operation = lambda: self._generate_cues(
            cues,
            task.signals.progress.emit,
            lambda: task.cancel_requested,
        )
        task.signals.progress.connect(self._generation_progress)
        self._start_task(task, f"กำลังสร้างเสียง 0 จาก {len(cues)} รายการ", self._after_generation, cancellable=True)

    def _start_task(self, task: BackgroundTask, message: str, on_result, cancellable: bool) -> None:
        if self._busy:
            return
        self._busy = True
        self.active_task = task
        self._last_completion_message = ""
        for action in self.busy_actions:
            action.setEnabled(False)
        self.task_label.setText(message)
        self.task_progress.setRange(0, 0)
        self.task_progress.setTextVisible(False)
        self.cancel_task_button.setVisible(cancellable)
        self.cancel_task_button.setEnabled(cancellable)
        self.task_panel.show()
        self.statusBar().showMessage(message)
        task.signals.result.connect(on_result)
        task.signals.error.connect(self._error)
        task.signals.finished.connect(self._task_finished)
        self.thread_pool.start(task)

    def _generation_progress(self, completed: int, total: int, cue_id: str) -> None:
        self.task_progress.setRange(0, max(1, total))
        self.task_progress.setValue(completed)
        self.task_progress.setTextVisible(True)
        elapsed = max(0.1, time.monotonic() - self._generation_started_at)
        eta_text = "กำลังประเมินเวลา"
        if completed:
            remaining_seconds = elapsed / completed * max(0, total - completed)
            if remaining_seconds >= 3600:
                eta_text = f"เหลือประมาณ {remaining_seconds / 3600:.1f} ชม."
            elif remaining_seconds >= 60:
                eta_text = f"เหลือประมาณ {remaining_seconds / 60:.0f} นาที"
            else:
                eta_text = f"เหลือประมาณ {remaining_seconds:.0f} วินาที"
        message = f"ดำเนินการแล้ว {completed} จาก {total} · {cue_id} · {eta_text}"
        self.task_label.setText(message)
        self.statusBar().showMessage(message)

        if self.project:
            cue = next((item for item in self.project.cues if item.id == cue_id), None)
            if cue is not None:
                row = self.project.cues.index(cue)
                if self.table.item(row, 3):
                    self.table.item(row, 3).setText(cue.selected_take_id or "—")
                if self.table.item(row, 4):
                    self.table.item(row, 4).setText(f"{cue.generated_duration / 1000:.2f}s" if cue.generated_duration else "—")
                if self.table.item(row, 6):
                    label = "กำลังสร้าง" if cue.status == CueStatus.GENERATING.value else "พร้อม"
                    self.table.item(row, 6).setText(label)
                    self.table.item(row, 6).setForeground(QColor("#78d9b3") if cue.status == CueStatus.READY.value else QColor("#b9c2ff"))

    def cancel_active_task(self) -> None:
        if self.active_task is None:
            return
        self.active_task.cancel()
        self.cancel_task_button.setEnabled(False)
        self.task_label.setText("รับคำสั่งหยุดแล้ว · จะหยุดหลังรายการปัจจุบันเสร็จ")
        self.statusBar().showMessage("กำลังหยุดอย่างปลอดภัยหลังรายการปัจจุบัน…")

    def _task_finished(self) -> None:
        self._busy = False
        self.active_task = None
        for action in self.busy_actions:
            action.setEnabled(True)
        self.task_panel.hide()
        self.statusBar().showMessage(self._last_completion_message or "พร้อม", 8000 if self._last_completion_message else 3000)
        self._refresh()

    def _error(self, message: str) -> None:
        if "JaiTTS runtime" in message or "JaiTTS dependency" in message:
            box = QMessageBox(self)
            box.setIcon(QMessageBox.Critical)
            box.setWindowTitle("ต้องตั้งค่า AI Runtime")
            box.setText("DubFlow ยังเปิดระบบสร้างเสียงไม่ได้")
            box.setInformativeText(
                f"{message}\n\nไม่ต้องดาวน์โหลดโมเดลหรือย้ายไฟล์เอง\n"
                "ไปที่ ตั้งค่า → AI Runtime แล้วเลือกโฟลเดอร์ Python .venv ที่มี PyTorch และ f5-tts "
                "จากนั้นปิดและเปิด DubFlow ใหม่\n\n"
                "โมเดลจะดาวน์โหลดอัตโนมัติไปที่ Workspace\\models เมื่อสร้างเสียงครั้งแรก"
            )
            settings_button = box.addButton("เปิดตั้งค่า", QMessageBox.AcceptRole)
            box.addButton(QMessageBox.Close)
            box.exec()
            if box.clickedButton() is settings_button:
                self.open_settings()
            return
        QMessageBox.critical(self, "DubFlow", message)

    def closeEvent(self, event) -> None:
        if self._busy:
            event.ignore()
            QMessageBox.information(self, "DubFlow", "กรุณาหยุดงานและรอให้รายการปัจจุบันเสร็จก่อนปิดโปรแกรม")
            return
        if self.project is not None and self.project_dir is not None:
            self.repository.save(self.project, self.project_dir)
        self.provider.unload()
        self.transcript_verifier.unload()
        super().closeEvent(event)
