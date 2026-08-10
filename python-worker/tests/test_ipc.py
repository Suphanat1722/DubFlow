import json

import pytest

from dubflow_worker.ipc import (
    PROTOCOL_VERSION,
    RpcResponse,
    encode_response,
    parse_request,
)


def test_parse_request_accepts_current_version():
    request = parse_request(
        json.dumps(
            {
                "protocolVersion": PROTOCOL_VERSION,
                "id": 7,
                "method": "system.ping",
                "params": {},
            }
        )
    )
    assert request.id == 7
    assert request.method == "system.ping"


def test_parse_request_rejects_unknown_version():
    with pytest.raises(ValueError, match="unsupported protocol version"):
        parse_request(
            json.dumps(
                {
                    "protocolVersion": 999,
                    "id": 1,
                    "method": "system.ping",
                    "params": {},
                }
            )
        )


def test_encode_response_round_trips():
    payload = json.loads(
        encode_response(RpcResponse(id=3, result={"pong": True}))
    )
    assert payload == {"id": 3, "result": {"pong": True}}


def test_encode_error_response():
    payload = json.loads(
        encode_response(
            RpcResponse(id=3, error={"code": -32601, "message": "boom"})
        )
    )
    assert payload["error"]["message"] == "boom"
