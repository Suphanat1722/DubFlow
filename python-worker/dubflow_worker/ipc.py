"""Versioned JSON-RPC envelope for the DubFlow Python worker.

Wire format: one JSON object per line on stdin/stdout. Every request carries a
`protocolVersion` and every response echoes the request `id` so the Rust shell
can correlate long-running generation jobs.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

PROTOCOL_VERSION = 1


@dataclass(frozen=True)
class RpcRequest:
    id: int
    method: str
    params: dict[str, Any]
    protocol_version: int = PROTOCOL_VERSION


@dataclass(frozen=True)
class RpcResponse:
    id: int
    result: dict[str, Any] | None = None
    error: dict[str, Any] | None = None


def parse_request(line: str) -> RpcRequest:
    """Parse a single JSON-RPC request line.

    Raises ValueError when the payload is malformed or the protocol version is
    not supported.
    """
    import json

    raw = json.loads(line)
    if raw.get("protocolVersion") != PROTOCOL_VERSION:
        raise ValueError(
            f"unsupported protocol version: {raw.get('protocolVersion')}"
        )
    return RpcRequest(
        id=raw["id"],
        method=raw["method"],
        params=raw.get("params") or {},
    )


def encode_response(response: RpcResponse) -> str:
    """Serialize a response to a single JSON line."""
    import json

    payload: dict[str, Any] = {"id": response.id}
    if response.error is not None:
        payload["error"] = response.error
    else:
        payload["result"] = response.result or {}
    return json.dumps(payload, ensure_ascii=False)
