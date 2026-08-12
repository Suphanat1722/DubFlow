from __future__ import annotations

import csv
import io
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


_DLL_HANDLES: list[object] = []


def runtime_site_packages(runtime_root: str | Path) -> Path | None:
    """Resolve a user-managed Python environment to its site-packages folder."""
    root = Path(runtime_root).expanduser()
    if root.is_file():
        root = root.parent.parent if root.parent.name.lower() == "scripts" else root.parent
    candidates = (root, root / "Lib" / "site-packages")
    return next((candidate.resolve() for candidate in candidates if candidate.is_dir() and (candidate / "torch").exists()), None)


def activate_runtime(runtime_root: str | Path) -> Path | None:
    """Expose an external inference runtime to a lightweight frozen GUI."""
    if not runtime_root:
        return None
    site_packages = runtime_site_packages(runtime_root)
    if site_packages is None:
        return None
    site_text = str(site_packages)
    if site_text not in sys.path:
        sys.path.insert(0, site_text)
    if os.name == "nt":
        directory = site_packages / "torch" / "lib"
        if directory.is_dir():
            try:
                _DLL_HANDLES.append(os.add_dll_directory(str(directory)))
            except OSError:
                pass
    return site_packages


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
        if self.runtime_root and activate_runtime(self.runtime_root) is None:
            return RuntimeInfo("Runtime ไม่พร้อม", detail="โฟลเดอร์ Runtime ไม่มี PyTorch")
        executable = shutil.which("nvidia-smi")
        if not executable:
            return RuntimeInfo("CPU fallback", detail="ไม่พบ NVIDIA GPU")
        result = subprocess.run([executable, "--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"], capture_output=True, text=True, check=False)
        if result.returncode:
            return RuntimeInfo("CPU fallback", detail="เรียก nvidia-smi ไม่สำเร็จ")
        row = next(csv.reader(io.StringIO(result.stdout)))
        name, memory, driver = (item.strip() for item in row[:3])
        try:
            import torch
            cuda_available = bool(torch.cuda.is_available())
        except (ImportError, OSError):
            cuda_available = False
        vram = int(memory)
        mode = "Legacy NVIDIA Runtime" if vram <= 8192 else "Modern NVIDIA Runtime"
        detail = "PyTorch CUDA พร้อมใช้" if cuda_available else "พบ GPU แต่ยังไม่มี PyTorch CUDA runtime"
        return RuntimeInfo(mode, name, vram, driver, cuda_available, detail)
