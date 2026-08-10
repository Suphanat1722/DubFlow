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
    """Now sends configure first, then synthesize without initialize."""
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 3,
                    "method": "worker.configure",
                    "params": {"outputDir": "C:/tmp"},
                }
            ),
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 4,
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
    # Second line is the synthesize error.
    payload = json.loads(stdout.strip().splitlines()[1])
    assert payload["error"]["message"] == "tts.initialize must be called first"


def test_worker_configure_sets_output_dir():
    import os
    import tempfile

    out_dir = tempfile.mkdtemp(prefix="dubflow-rt-")
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 1,
                    "method": "worker.configure",
                    "params": {"outputDir": out_dir},
                }
            )
        ]
    )
    payload = json.loads(stdout.splitlines()[0])
    assert payload["result"]["outputDir"] == out_dir


def test_worker_requires_configure_before_synthesis():
    """Call synthesize without configure -> outputDir error (checked before initialize)."""
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 6,
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
            ),
        ]
    )
    payload = json.loads(stdout.strip().splitlines()[0])
    assert payload["error"]["message"] == "worker.configure with outputDir must be called before synthesis"
    assert payload["error"]["kind"] == ""


def test_worker_error_has_kind_for_missing_file():
    """Test the pure error_payload function directly (no GPU needed)."""
    from dubflow_worker.worker import error_payload
    from dubflow_worker.tts import TtsError

    payload = error_payload(FileNotFoundError("C:/missing.wav"))
    assert payload["kind"] == "missing-file"
    assert payload["code"] == -32601

    payload = error_payload(TtsError("no-audio", "synthesis produced no audio"))
    assert payload["kind"] == "no-audio"
    assert payload["code"] == -32601

    payload = error_payload(RuntimeError("generic"))
    assert payload["kind"] == ""


def test_worker_reports_close_without_initialize():
    stdout = run_worker(
        [
            json.dumps(
                {
                    "protocolVersion": 1,
                    "id": 5,
                    "method": "tts.close",
                    "params": {},
                }
            )
        ]
    )
    assert json.loads(stdout.splitlines()[0]) == {
        "id": 5,
        "result": {"closed": True},
    }
