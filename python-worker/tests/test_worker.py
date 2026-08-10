import json
import subprocess
import sys


def run_worker(lines: list[str]) -> str:
    proc = subprocess.run(
        [sys.executable, "-m", "dubflow_worker.worker"],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        cwd=".",
    )
    assert proc.returncode == 0, proc.stderr
    return proc.stdout


def test_worker_ping_round_trip():
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 1,
                    "method": "system.ping",
                    "params": {},
                }
            )
        ]
    )
    assert json.loads(stdout.splitlines()[0]) == {
        "id": 1,
        "result": {"pong": True},
    }


def test_worker_returns_error_for_unknown_method():
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 2,
                    "method": "no.such.method",
                    "params": {},
                }
            )
        ]
    )
    assert json.loads(stdout.splitlines()[0])["error"]["message"] == (
        "unknown method: no.such.method"
    )


def test_worker_requires_initialize_before_synthesis():
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 3,
                    "method": "tts.synthesize",
                    "params": {
                        "reference": {
                            "audioPath": "x.wav",
                            "transcript": "x",
                            "durationMs": 1000,
                            "sampleRate": 24000,
                            "sha256": "0" * 64,
                        },
                        "text": "hello",
                        "seed": 1,
                    },
                }
            )
        ]
    )
    payload = json.loads(stdout.splitlines()[0])
    assert payload["error"]["message"] == "tts.initialize must be called first"


def test_worker_reports_close_without_initialize():
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 4,
                    "method": "tts.close",
                    "params": {},
                }
            )
        ]
    )
    assert json.loads(stdout.splitlines()[0]) == {
        "id": 4,
        "result": {"closed": True},
    }
