from __future__ import annotations

import csv
import io
import shutil
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class RuntimeInfo:
    mode: str
    gpu_name: str = ""
    vram_mb: int = 0
    driver_version: str = ""
    cuda_available: bool = False
    detail: str = ""


class RuntimeManager:
    def detect(self) -> RuntimeInfo:
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
        except ImportError:
            cuda_available = False
        vram = int(memory)
        mode = "Legacy NVIDIA Runtime" if vram <= 8192 else "Modern NVIDIA Runtime"
        detail = "PyTorch CUDA พร้อมใช้" if cuda_available else "พบ GPU แต่ยังไม่มี PyTorch CUDA runtime"
        return RuntimeInfo(mode, name, vram, driver, cuda_available, detail)
