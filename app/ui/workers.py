from __future__ import annotations

from collections.abc import Callable
from typing import Any

from PySide6.QtCore import QObject, QRunnable, Signal, Slot


class WorkerSignals(QObject):
    result = Signal(object)
    error = Signal(str)
    progress = Signal(int, int, str)
    finished = Signal()


class BackgroundTask(QRunnable):
    def __init__(self, operation: Callable[[], Any]):
        super().__init__()
        self.operation = operation
        self.signals = WorkerSignals()
        self.cancel_requested = False

    def cancel(self) -> None:
        self.cancel_requested = True

    @Slot()
    def run(self) -> None:
        try:
            self.signals.result.emit(self.operation())
        except Exception as exc:
            self.signals.error.emit(str(exc) or exc.__class__.__name__)
        finally:
            self.signals.finished.emit()
