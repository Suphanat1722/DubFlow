from __future__ import annotations

import csv
import io
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path


def runtime_site_packages(runtime_root: str | Path) -> Path | None:
    """Resolve a user-managed Python environment to its site-packages folder."""
    root = Path(runtime_root).expanduser()
    if root.is_file():
        root = root.parent.parent if root.parent.name.lower() == "scripts" else root.parent
    candidates = (root, root / "Lib" / "site-packages")
    return next((candidate.resolve() for candidate in candidates if candidate.is_dir() and (candidate / "torch").exists()), None)


def runtime_python(runtime_root: str | Path, windowed: bool = False) -> Path | None:
    """Return the interpreter from a complete user-managed Python environment."""
    if not runtime_root:
        return None
    root = Path(runtime_root).expanduser()
    if root.is_file():
        root = root.parent.parent if root.parent.name.lower() == "scripts" else root.parent
    name = "pythonw.exe" if windowed else "python.exe"
    candidate = root / "Scripts" / name
    if not candidate.is_file() or runtime_site_packages(root) is None:
        return None
    return candidate.resolve()


@dataclass(frozen=True)
class RuntimeInfo:
    mode: str
    gpu_name: str = ""
    vram_mb: int = 0
    driver_version: str = ""
    cuda_available: bool = False
    detail: str = ""


class RuntimeManager:
    def __init__(self, runtime_root: str = ""):
        self.runtime_root = runtime_root

    def detect(self) -> RuntimeInfo:
        if self.runtime_root and runtime_python(self.runtime_root) is None:
            return RuntimeInfo("Runtime ไม่พร้อม", detail="โฟลเดอร์นี้ไม่มี Python หรือ PyTorch ที่ใช้งานได้")
        executable = shutil.which("nvidia-smi")
        if not executable:
            return RuntimeInfo("CPU fallback", detail="ไม่พบ NVIDIA GPU")
        result = subprocess.run([executable, "--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"], capture_output=True, text=True, check=False)
        if result.returncode:
            return RuntimeInfo("CPU fallback", detail="เรียก nvidia-smi ไม่สำเร็จ")
        row = next(csv.reader(io.StringIO(result.stdout)))
        name, memory, driver = (item.strip() for item in row[:3])
        if self.runtime_root:
            python = runtime_python(self.runtime_root)
            check = subprocess.run([str(python), "-c", "import torch;print(int(torch.cuda.is_available()))"], capture_output=True, text=True, check=False)
            cuda_available = check.returncode == 0 and check.stdout.strip() == "1"
        else:
            try:
                import torch
                cuda_available = bool(torch.cuda.is_available())
            except (ImportError, OSError):
                cuda_available = False
        vram = int(memory)
        mode = "Legacy NVIDIA Runtime" if vram <= 8192 else "Modern NVIDIA Runtime"
        detail = "PyTorch CUDA พร้อมใช้" if cuda_available else "พบ GPU แต่ยังไม่มี PyTorch CUDA runtime"
        return RuntimeInfo(mode, name, vram, driver, cuda_available, detail)
