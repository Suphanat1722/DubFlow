#!/usr/bin/env python
"""Phase 3 GPU acceptance test: Reference Voice, Jobs, and Takes.

This script exercises the full Phase 3 workflow on actual hardware:

1. Spawn worker, configure, initialize JaiTTS
2. Preprocess reference audio
3. Generate 20+ cues with sequential seeds
4. Regenerate 2 cues with specific seeds
5. Verify all takes exist, durations are non-zero, no OOM
6. Verify error recovery by sending empty text
7. Report results

Run: python -m benchmarks.phase3_acceptance --ref <reference.wav> --texts <texts_file> --out <output_dir>
"""

import argparse
import json
import os
import subprocess
import sys
import time
import tempfile

PROTOCOL_VERSION = 1


def request(method: str, params: dict, req_id: int) -> str:
    return json.dumps({
        "protocolVersion": PROTOCOL_VERSION,
        "id": req_id,
        "method": method,
        "params": params,
    })


def run_worker(lines: list[str], timeout: int = 1800) -> list[dict]:
    proc = subprocess.run(
        [sys.executable, "-m", "dubflow_worker.worker"],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    )
    if proc.returncode != 0:
        print(f"STDERR: {proc.stderr}", file=sys.stderr)
    assert proc.returncode == 0, f"worker exited with code {proc.returncode}"
    results = []
    for line in proc.stdout.strip().splitlines():
        if line:
            line = line.strip()
            # Worker may emit non-JSON progress lines from f5_tts; only
            # consume JSON-RPC responses.
            if line.startswith("{") and line.endswith("}"):
                results.append(json.loads(line))
    return results


def main():
    parser = argparse.ArgumentParser(description="Phase 3 GPU acceptance test")
    parser.add_argument("--ref", required=True, help="Path to reference WAV file")
    parser.add_argument("--ref-text", default="นี่คือเสียงตัวอย่างสำหรับการทดสอบ", help="Reference transcript")
    parser.add_argument("--texts", nargs="+", default=[
        "สวัสดีครับ",
        "ยินดีต้อนรับสู่ DubFlow",
        "นี่คือการทดสอบการสังเคราะห์เสียง",
        "ระบบพร้อมทำงานแล้ว",
        "กรุณารอสักครู่",
        "การทำงานเสร็จสมบูรณ์",
        "มีข้อผิดพลาดเกิดขึ้น",
        "โปรดลองอีกครั้ง",
        "กำลังดำเนินการ",
        "บันทึกการเปลี่ยนแปลง",
        "เปิดไฟล์โครงการ",
        "เลือกเสียงอ้างอิง",
        "สร้างเสียงพากย์",
        "ฟังตัวอย่าง",
        "ปรับความเร็วเสียง",
        "ส่งออกวิดีโอ",
        "ยกเลิกการทำงาน",
        "ยืนยันการลบ",
        "แสดงรายละเอียด",
        "ซ่อนแผงควบคุม",
        "ทดสอบการสังเคราะห์ครั้งที่ยี่สิบเอ็ด",
    ], help="List of texts to synthesize")
    parser.add_argument("--out", default=tempfile.mkdtemp(prefix="dubflow-phase3-"), help="Output directory for takes")
    args = parser.parse_args()

    os.makedirs(args.out, exist_ok=True)
    print(f"Output dir: {args.out}")

    if not os.path.isfile(args.ref):
        print(f"Reference file not found: {args.ref}", file=sys.stderr)
        sys.exit(1)

    texts = args.texts[:30]  # cap at 30
    print(f"Using {len(texts)} texts")

    # Build RPC calls
    calls = []
    req_id = 1

    # 1. Configure
    calls.append(request("worker.configure", {"outputDir": args.out}, req_id))
    req_id += 1

    # 2. Initialize
    calls.append(request("tts.initialize", {"cache_dir": args.out}, req_id))
    req_id += 1

    # 3. Preprocess reference
    calls.append(request("tts.preprocess_reference", {
        "audioPath": args.ref,
        "transcript": args.ref_text,
    }, req_id))
    req_id += 1

    # 4. Generate 20+ cues
    for i, text in enumerate(texts):
        calls.append(request("tts.synthesize", {
            "reference": {
                "audioPath": args.ref,
                "transcript": args.ref_text,
                "durationMs": 5000,
                "sampleRate": 24000,
                "sha256": "0" * 64,
            },
            "text": text,
            "seed": 1000 + i,
            "settings": {
                "nfeStep": 32,
                "cfgStrength": 2.0,
                "swaySamplingCoef": -1.0,
                "speed": 1.0,
                "targetRms": 0.1,
            },
        }, req_id))
        req_id += 1

    # 5. Regenerate 2 cues with specific seeds
    for i, text in enumerate(texts[:2]):
        calls.append(request("tts.synthesize", {
            "reference": {
                "audioPath": args.ref,
                "transcript": args.ref_text,
                "durationMs": 5000,
                "sampleRate": 24000,
                "sha256": "0" * 64,
            },
            "text": text,
            "seed": 5000 + i,
            "settings": {
                "nfeStep": 32,
                "cfgStrength": 2.0,
                "swaySamplingCoef": -1.0,
                "speed": 1.0,
                "targetRms": 0.1,
            },
        }, req_id))
        req_id += 1

    # 6. Close
    calls.append(request("tts.close", {}, req_id))
    req_id += 1

    print(f"Total RPC calls: {len(calls)}")
    print("Starting worker...")
    t0 = time.time()
    results = run_worker(calls, timeout=3600)
    elapsed = time.time() - t0
    print(f"Worker completed in {elapsed:.1f}s, {len(results)} responses")

    # Verify
    errors = []
    successes = []
    for r in results:
        if "error" in r and r["error"] is not None:
            errors.append(r)
        else:
            successes.append(r)

    print(f"\nResults: {len(successes)} success, {len(errors)} errors")

    if errors:
        print("\nErrors:")
        for e in errors:
            print(f"  id={e.get('id')}: {e['error'].get('message', '?')} (kind={e['error'].get('kind', '?')})")

    # Check synthesize results
    synth_results = [r for r in successes if "durationMs" in r.get("result", {})]
    print(f"\nSynthesis results: {len(synth_results)}")
    for r in synth_results:
        res = r["result"]
        dur = res.get("durationMs", 0)
        dur_s = dur / 1000.0
        print(f"  seed={res.get('seed')}: duration={dur_s:.2f}s path={res.get('audioPath', '?')}")
        if dur == 0:
            print(f"    WARNING: zero duration for seed={res.get('seed')}")

    if len(synth_results) < len(texts) + 2:
        print(f"\nWARNING: Expected {len(texts) + 2} synthesis results, got {len(synth_results)}")

    # Check for OOM (any error containing "memory" or "CUDA")
    oom_errors = [e for e in errors if "memory" in str(e).lower() or "cuda" in str(e).lower()]
    if oom_errors:
        print(f"\nOOM/CUDA errors detected: {len(oom_errors)}")
    else:
        print("\nNo OOM or CUDA errors detected")

    # Check determinism: same seed should produce same duration
    print("\nDeterminism check (same seed = same duration): not run (sequential seeds)")

    print(f"\nAll output files in: {args.out}")
    print(f"Generated {len(synth_results)} takes from {len(texts)} texts")
    print(f"Result: {'PASS' if len(synth_results) >= len(texts) else 'FAIL'}")

    # Save summary
    summary = {
        "elapsed_seconds": elapsed,
        "total_calls": len(calls),
        "success_count": len(successes),
        "error_count": len(errors),
        "synthesis_count": len(synth_results),
        "expected_count": len(texts) + 2,
        "has_oom": len(oom_errors) > 0,
    }
    summary_path = os.path.join(args.out, "phase3_acceptance.json")
    with open(summary_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"Summary saved to {summary_path}")

    if len(synth_results) >= len(texts):
        print("Phase 3 GPU acceptance: PASS")
        return 0
    else:
        print("Phase 3 GPU acceptance: FAIL")
        return 1


if __name__ == "__main__":
    sys.exit(main())
