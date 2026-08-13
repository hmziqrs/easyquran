export const FETCH_TIMEOUT_MS = 3000;
export const RANGE_CHUNK_TIMEOUT_MS = 10_000;
export const LOCAL_BOOT_BUDGET_MS = 1500;
export const RESPONSE_CAP = 300;

export async function fetchWithTimeout(
  url: string,
  init?: RequestInit & { timeout?: number },
): Promise<Response> {
  const timeout = init?.timeout ?? FETCH_TIMEOUT_MS;
  const external = init?.signal;
  const ctrl = new AbortController();
  const onAbort = () => ctrl.abort();
  if (external) {
    if (external.aborted) ctrl.abort();
    else external.addEventListener("abort", onAbort, { once: true });
  }
  const timer = setTimeout(() => ctrl.abort(), timeout);
  try {
    return await fetch(url, { ...init, signal: ctrl.signal });
  } finally {
    clearTimeout(timer);
    if (external) external.removeEventListener("abort", onAbort);
  }
}

export class FetchHttpError extends Error {
  readonly status: number;
  constructor(status: number) {
    super(`http ${status}`);
    this.name = "FetchHttpError";
    this.status = status;
  }
}

export class FetchTimeoutError extends Error {
  constructor() {
    super("fetch timed out");
    this.name = "FetchTimeoutError";
  }
}

export class MalformedDataError extends Error {
  constructor(message = "malformed response") {
    super(message);
    this.name = "MalformedDataError";
  }
}

export type JsonDocument =
  | string
  | number
  | boolean
  | null
  | readonly JsonDocument[]
  | { readonly [key: string]: JsonDocument };

export async function fetchJsonWithTimeout(
  url: string,
  init?: RequestInit & { timeout?: number },
): Promise<JsonDocument> {
  const timeout = init?.timeout ?? FETCH_TIMEOUT_MS;
  const external = init?.signal;
  const ctrl = new AbortController();
  const onAbort = () => ctrl.abort();
  if (external) {
    if (external.aborted) ctrl.abort();
    else external.addEventListener("abort", onAbort, { once: true });
  }
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    ctrl.abort();
  }, timeout);
  try {
    const res = await fetch(url, { ...init, signal: ctrl.signal });
    if (!res.ok) throw new FetchHttpError(res.status);
    return await res.json();
  } catch (e) {
    if (timedOut) throw new FetchTimeoutError();
    throw e;
  } finally {
    clearTimeout(timer);
    if (external) external.removeEventListener("abort", onAbort);
  }
}

export type ReadFailureKind = "timeout" | "transport" | "http" | "malformed" | "worker";

export interface ReadFailure {
  kind: ReadFailureKind;
  status?: number;
}

export interface ReadTierStatus {
  servedBy: "local" | "api";
  workerFailure?: ReadFailure;
  apiFailure?: ReadFailure;
}

export class ReadChainError extends Error {
  readonly workerFailure?: ReadFailure;
  readonly apiFailure?: ReadFailure;
  constructor(message: string, workerFailure?: ReadFailure, apiFailure?: ReadFailure) {
    super(message);
    this.name = "ReadChainError";
    this.workerFailure = workerFailure;
    this.apiFailure = apiFailure;
  }
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); instanceof narrowing below is the parse
export function classifyApiFailure(e: unknown): ReadFailure {
  if (e instanceof FetchTimeoutError) return { kind: "timeout" };
  if (e instanceof FetchHttpError) return { kind: "http", status: e.status };
  if (e instanceof MalformedDataError) return { kind: "malformed" };
  return { kind: "transport" };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); classification is kind-level only
export function classifyWorkerFailure(_e: unknown): ReadFailure {
  return { kind: "worker" };
}
