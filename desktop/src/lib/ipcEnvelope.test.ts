import { describe, expect, it } from "vitest";

import { buildRpcRequest, isRpcResponse } from "./ipcEnvelope";

describe("ipcEnvelope", () => {
  it("builds a versioned request", () => {
    expect(buildRpcRequest(1, "system.ping", {})).toEqual({
      protocolVersion: 1,
      id: 1,
      method: "system.ping",
      params: {},
    });
  });

  it("recognizes a response with result", () => {
    expect(isRpcResponse({ id: 1, result: { pong: true } })).toBe(true);
  });

  it("recognizes a response with error", () => {
    expect(
      isRpcResponse({ id: 1, error: { code: -32601, message: "boom" } }),
    ).toBe(true);
  });

  it("rejects malformed payloads", () => {
    expect(isRpcResponse({ id: 1 })).toBe(false);
    expect(isRpcResponse("nope")).toBe(false);
  });
});
