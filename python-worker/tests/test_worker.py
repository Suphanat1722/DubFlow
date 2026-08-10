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
