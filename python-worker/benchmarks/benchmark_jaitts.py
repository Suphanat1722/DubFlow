"""GPU feasibility benchmark for the JaiTTS-F5TTS adapter.

Usage:
    python -m benchmarks.benchmark_jaitts --ref <wav> --text "<transcript>" --out <dir>

Measures cold load time, VRAM peak, generation time/RTF, output duration,
determinism, a 30-cue stress run, and worker error recovery.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
import threading
from dataclasses import asdict
from pathlib import Path

os.environ.setdefault("PYTHONIOENCODING", "utf-8")
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from dubflow_worker.tts import (
    ReferenceArtifact,
    ReferenceInput,
    SynthesisRequest,
    SynthesisSettings,
)
from dubflow_worker.tts.jaitts_f5tts import JaittsF5TtsProvider


def _vram_mb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    return torch.cuda.memory_allocated() / 1024 / 1024


def _vram_reserved_mb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    return torch.cuda.memory_reserved() / 1024 / 1024


def _sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _rms_db(path: str) -> float:
    import numpy as np
    import soundfile as sf

    data, _ = sf.read(path, dtype="float32")
    rms = float(np.sqrt(np.mean(np.square(data))))
    if rms <= 0:
        return -120.0
    return 20.0 * np.log10(rms)


class NvidiaSmiSampler:
    def __init__(self) -> None:
        self._stop = threading.Event()
        self._peak_mib = 0.0
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        if not torch.cuda.is_available():
            return
        self._peak_mib = 0.0
        self._thread = threading.Thread(target=self._sample, daemon=True)
        self._thread.start()

    def _sample(self) -> None:
        query = "nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits"
        while not self._stop.is_set():
            try:
                proc = subprocess.run(
                    query,
                    shell=True,
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                if proc.returncode == 0 and proc.stdout.strip():
                    used = float(proc.stdout.strip().splitlines()[0])
                    self._peak_mib = max(self._peak_mib, used)
            except Exception:
                pass
            time.sleep(0.5)

    def stop(self) -> float:
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=5)
        return self._peak_mib


def _readable_samples() -> list[tuple[str, float]]:
    # Vary text lengths so cue duration covers the 1-15s requirement.
    phrases = [
        "สวัสดีครับ ยินดีต้อนรับเข้าสู่ DubFlow",
        "วันนี้อากาศดีมาก เราจะไปเที่ยวทะเลกัน",
        "กรุณารอสักครู่ ระบบกำลังประมวลผลเสียงของคุณ",
        "ฉันอยากได้กาแฟร้อนหนึ่งแก้ว และขนมปังอีกสองชิ้น",
        "เรื่องราวเริ่มต้นเมื่อนานมาแล้วในเมืองเล็ก ๆ ริมแม่น้ำ",
        "คุณสามารถกดปุ่มเพื่อสร้างเสียงพากย์ใหม่ได้ทันที",
        "ทุกประโยคจะถูกจัดตำแหน่งบนไทม์ไลน์โดยอัตโนมัติ",
        "ถ้าเสียงยาวเกินไป เราจะเร่งความเร็วเล็กน้อยโดยไม่เปลี่ยนระดับเสียง",
        "เมื่อทุกอย่างพร้อมแล้ว กดส่งออกเพื่อรวมเสียงกับวิดีโอ",
        "นี่คือตัวอย่างประโยคที่สิบสำหรับการทดสอบความต่อเนื่อง",
        "ตอนนี้เรากำลังทดสอบการสังเคราะห์เสียงบนการ์ดจอ GTX 1070 Ti",
        "เสียงพากย์ที่ได้จะถูกบันทึกเป็นไฟล์ WAV ความถี่ยี่สิบสี่กิโลเฮิรตซ์",
        "การสร้างเสียงหนึ่งประโยคใช้เวลาน้อยกว่าหนึ่งนาทีในเครื่องทดสอบนี้",
        "กรุณาตรวจสอบความดังของเสียงก่อนเลือกเก็บเป็นผลงานขั้นสุดท้าย",
        "ถ้าต้องการเสียงที่เร็วขึ้น ให้ปรับความเร็วโดยไม่เปลี่ยนระดับเสียง",
        "โปรเจกต์นี้รองรับเฉพาะวิดีโอ MP4 และไฟล์คำบรรยายแบบ UTF-8",
        "ระบบจะแสดงสถานะของทุกบรรทัดบนหน้าจอเพื่อให้ตรวจทานง่าย",
        "การเปลี่ยนเสียงพากย์ไม่ทำให้ไฟล์เสียงเดิมสูญหาย",
        "ผู้ใช้สามารถฟังตัวอย่างแล้วเลือก Take ที่ถูกใจได้ทันที",
        "การส่งออกเสียงท้ายสุดจะมีความยาวเท่ากับวิดีโอต้นฉบับ",
        "การทดสอบนี้รันบนวินโดวส์ด้วยภาษาไทยเป็นหลัก",
        "เราวัดความเร็วการสร้างเสียงด้วยค่าเรียลไทม์แฟกเตอร์",
        "หน่วยความจำการ์ดจอถูกตรวจสอบตลอดการทดสอบทั้งหมด",
        "เสียงที่เงียบเกินไปจะถูกตรวจพบและรายงานให้ผู้ใช้ทราบ",
        "การสร้างเสียงซ้ำด้วย seed เดียวกันให้ผลเหมือนเดิม",
        "ถ้าเกิดข้อผิดพลาด ระบบจะกู้คืนและทำงานต่อได้",
        "ไฟล์เสียงต้นฉบับจะไม่ถูกแก้ไขเมื่อสร้าง Take ใหม่",
        "แอปพลิเคชันนี้ทำงานออฟไลน์หลังจากดาวน์โหลดโมเดลครั้งแรก",
        "ตัวอย่างนี้ใช้สำหรับทดสอบความต่อเนื่องของการสร้างเสียง",
        "สุดท้ายนี้ ขอบคุณที่ทดสอบ DubFlow ในช่วงพัฒนา",
    ]
    result: list[tuple[str, float]] = []
    for i, phrase in enumerate(phrases[:30]):
        result.append((phrase, float(3 + (i % 5) * 2.5)))
    return result


def _run_worker_roundtrip(request_line: str) -> dict:
    proc = subprocess.run(
        [sys.executable, "-m", "dubflow_worker.worker"],
        input=request_line + "\n",
        capture_output=True,
        text=True,
        cwd=str(Path(__file__).resolve().parents[1]),
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr)
    line = proc.stdout.strip().splitlines()[0]
    return json.loads(line)


def _run(provider, ref: ReferenceArtifact, text: str, seed: int, out_dir: Path) -> dict:
    t0 = time.monotonic()
    result = provider.synthesize(
        SynthesisRequest(
            reference=ref,
            text=text,
            seed=seed,
            settings=SynthesisSettings(speed=1.0),
            output_dir=str(out_dir),
        )
    )
    elapsed = time.monotonic() - t0
    duration_s = result.duration_ms / 1000.0
    return {
        "audioPath": result.audio_path,
        "audioSha256": _sha256_file(result.audio_path),
        "durationMs": result.duration_ms,
        "seed": result.seed,
        "sampleRate": result.sample_rate,
        "settingsHash": result.settings_hash,
        "rmsDb": _rms_db(result.audio_path),
        "wallSeconds": elapsed,
        "rtf": elapsed / duration_s if duration_s else 0.0,
        "vramAllocatedMiB": _vram_mb(),
        "vramReservedMiB": _vram_reserved_mb(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ref", required=True, help="reference audio WAV")
    parser.add_argument("--text", required=True, help="reference transcript")
    parser.add_argument("--out", required=True, help="benchmark output directory")
    parser.add_argument("--cache-dir", default=None, help="HF cache directory")
    args = parser.parse_args()

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = args.cache_dir or os.environ.get("HF_HOME")

    report: dict = {
        "gpu": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
        "computeCapability": list(torch.cuda.get_device_capability(0))
        if torch.cuda.is_available()
        else None,
        "torch": torch.__version__,
        "cudaAvailable": torch.cuda.is_available(),
    }

    # Cold load
    t0 = time.monotonic()
    provider = JaittsF5TtsProvider()
    provider.initialize({"cache_dir": cache_dir})
    report["coldLoadSeconds"] = time.monotonic() - t0
    report["vramAllocatedAfterLoadMiB"] = _vram_mb()
    report["vramReservedAfterLoadMiB"] = _vram_reserved_mb()

    ref = provider.preprocess_reference(ReferenceInput(args.ref, args.text))
    report["reference"] = asdict(ref)
    report["vramAllocatedAfterRefMiB"] = _vram_mb()

    # Determinism: same seed twice
    first = _run(provider, ref, "ทดสอบการกำหนดค่า seed ครั้งแรก", 12345, out_dir / "det-a")
    second = _run(provider, ref, "ทดสอบการกำหนดค่า seed ครั้งแรก", 12345, out_dir / "det-b")
    report["determinism"] = {
        "firstAudioSha256": first["audioSha256"],
        "secondAudioSha256": second["audioSha256"],
        "sameSeedSameAudio": first["audioSha256"] == second["audioSha256"],
        "sameSeedSameDuration": first["durationMs"] == second["durationMs"],
    }

    # Single-cue benchmark over varied lengths
    samples = _readable_samples()
    runs: list[dict] = []
    for i, (text, _) in enumerate(samples[:5]):
        runs.append(_run(provider, ref, text, 1000 + i, out_dir / "single"))
    report["singleCueRuns"] = runs
    report["singleCueMeanRtf"] = sum(r["rtf"] for r in runs) / len(runs)

    # 30-cue stress run
    stress: list[dict] = []
    sampler = NvidiaSmiSampler()
    sampler.start()
    peak_allocated = 0.0
    peak_reserved = 0.0
    for i, (text, _) in enumerate(samples):
        r = _run(provider, ref, text, 2000 + i, out_dir / "stress")
        stress.append(r)
        peak_allocated = max(peak_allocated, r["vramAllocatedMiB"])
        peak_reserved = max(peak_reserved, r["vramReservedMiB"])
    report["stressPeakNvidiaSmiMiB"] = sampler.stop()
    report["stress"] = stress
    report["stressPeakVramAllocatedMiB"] = peak_allocated
    report["stressPeakVramReservedMiB"] = peak_reserved
    report["stressTotalSeconds"] = sum(r["wallSeconds"] for r in stress)
    report["stressMeanRtf"] = sum(r["rtf"] for r in stress) / len(stress)
    report["stressMinDurationMs"] = min(r["durationMs"] for r in stress)
    report["stressMaxDurationMs"] = max(r["durationMs"] for r in stress)
    report["stressMinRmsDb"] = min(r["rmsDb"] for r in stress)

    # Error recovery: bad text should raise, then the provider still works.
    error_before = _vram_mb()
    try:
        provider.synthesize(
            SynthesisRequest(
                reference=ref,
                text="",
                seed=999,
                settings=SynthesisSettings(speed=1.0),
                output_dir=str(out_dir / "error"),
            )
        )
        report["errorRecovery"] = {"raised": False}
    except Exception as exc:
        report["errorRecovery"] = {
            "raised": True,
            "type": type(exc).__name__,
            "message": str(exc)[:300],
        }
        recovered = _run(provider, ref, "กู้คืนหลังข้อผิดพลาดแล้วทํางานต่อได้", 998, out_dir / "recovery")
        report["errorRecovery"]["recoveredAfterError"] = recovered
    report["vramAfterErrorMiB"] = _vram_mb()
    report["vramGrowthErrorMiB"] = _vram_mb() - error_before

    # Worker round-trip
    import json as _json

    probe = _json.dumps(
        {
            "protocolVersion": 1,
            "id": 1,
            "method": "system.ping",
            "params": {},
        }
    )
    report["workerRoundTrip"] = _run_worker_roundtrip(probe)

    provider.close()
    report["vramAllocatedAfterCloseMiB"] = _vram_mb()

    report_path = out_dir / "benchmark.json"
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
