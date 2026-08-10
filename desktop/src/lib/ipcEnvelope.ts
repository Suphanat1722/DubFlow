export const IPC_PROTOCOL_VERSION = 1;

export interface RpcRequest {
  protocolVersion: number;
  id: number;
  method: string;
  params: Record<string, unknown>;
}

export interface RpcError {
  code: number;
  message: string;
}

export interface RpcResponse {
  id: number;
  result?: Record<string, unknown>;
  error?: RpcError;
}

export function buildRpcRequest(
  id: number,
  method: string,
  params: Record<string, unknown>,
): RpcRequest {
  return {
    protocolVersion: IPC_PROTOCOL_VERSION,
    id,
    method,
    params,
  };
}

export function isRpcResponse(value: unknown): value is RpcResponse {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.id !== "number") return false;
  return (
    (candidate.result !== undefined && typeof candidate.result === "object") ||
    (candidate.error !== undefined && typeof candidate.error === "object")
  );
}
