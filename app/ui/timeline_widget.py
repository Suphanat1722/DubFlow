from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor, QPainter
from PySide6.QtWidgets import QWidget

from app.models import Cue, CueStatus


class TimelineWidget(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self._cues: list[Cue] = []
        self._duration = 1
        self._position = 0
        self.setMinimumHeight(84)

    def set_data(self, cues: list[Cue], duration_ms: int = 0) -> None:
        self._cues = cues
        last_end = max((cue.resolved_end or cue.original_end for cue in cues), default=1)
        self._duration = max(1, duration_ms, last_end)
        self.update()

    def set_position(self, position_ms: int) -> None:
        self._position = position_ms
        self.update()

    def paintEvent(self, event) -> None:
        del event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)
        painter.fillRect(self.rect(), QColor("#151923"))
        width = max(1, self.width() - 24)

        for cue in self._cues:
            x = 12 + round(cue.original_start / self._duration * width)
            original_width = max(2, round(cue.slot_duration / self._duration * width))
            painter.fillRect(x, 14, original_width, 20, QColor("#38445f"))
            if cue.generated_duration is not None:
                end = cue.resolved_end or cue.original_end
                generated_width = max(2, round((end - (cue.resolved_start or cue.original_start)) / self._duration * width))
                color = QColor("#ef6b73") if cue.status == CueStatus.NEEDS_REVIEW.value else QColor("#5bc99a")
                painter.fillRect(x, 46, generated_width, 20, color)

        playhead = 12 + round(self._position / self._duration * width)
        painter.setPen(QColor("#ffd166"))
        painter.drawLine(playhead, 6, playhead, self.height() - 6)
        painter.setPen(QColor("#9ba7bd"))
        painter.drawText(12, self.height() - 4, "SRT")
        painter.drawText(44, self.height() - 4, "Voice")
