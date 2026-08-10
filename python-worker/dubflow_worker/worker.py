"""Entry point for the DubFlow worker process."""

from __future__ import annotations

import json
import sys

from .ipc import RpcResponse, encode_response, parse_request


def handle(method: str, params: dict) -> dict:
    """Route a single RPC method. Phase 1 adds TTS methods here."""
    if method == "system.ping":
        return {"pong": True}
    raise ValueError(f"unknown method: {method}")


def run() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = parse_request(line)
            result = handle(request.method, request.params)
            response = RpcResponse(id=request.id, result=result)
        except Exception as exc:
            response = RpcResponse(
                id=getattr(exc, "id", -1),
                error={"code": -32601, "message": str(exc)},
            )
        sys.stdout.write(encode_response(response) + "\n")
        sys.stdout.flush()


def main() -> None:
    try:
        run()
    except (BrokenPipeError, KeyboardInterrupt):
        pass


if __name__ == "__main__":
    main()
