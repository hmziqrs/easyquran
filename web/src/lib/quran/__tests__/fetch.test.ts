import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import {
  classifyApiFailure,
  classifyWorkerFailure,
  FetchHttpError,
  FetchTimeoutError,
  fetchJsonWithTimeout,
  fetchWithTimeout,
  FETCH_TIMEOUT_MS,
  MalformedDataError,
  ReadChainError,
} from "$lib/quran/fetch";

function mockAbortAwareFetch(): ReturnType<typeof vi.spyOn> {
  return vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
    const sig = (init as RequestInit | undefined)?.signal;
    return new Promise<Response>((_resolve, reject) => {
      if (sig?.aborted) {
        reject(new DOMException("aborted", "AbortError"));
        return;
      }
      sig?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
    });
  });
}

describe("fetchWithTimeout", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("aborts through the default timeout and observes the inner signal aborted", async () => {
    let observed: AbortSignal | null | undefined;
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      observed = (init as RequestInit | undefined)?.signal;
      return new Promise<Response>((_resolve, reject) => {
        observed?.addEventListener("abort", () =>
          reject(new DOMException("aborted", "AbortError")),
        );
      });
    });
    const p = fetchWithTimeout("https://x.test/");
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(FETCH_TIMEOUT_MS);
    await expect(p).rejects.toThrow(/aborted/i);
    expect(observed?.aborted).toBe(true);
  });

  it("honours an explicit timeout override (499ms no throw, 500ms throws)", async () => {
    const spy = mockAbortAwareFetch();
    const p = fetchWithTimeout("https://x.test/", { timeout: 500 });
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(499);
    expect(spy).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    await expect(p).rejects.toThrow(/aborted/i);
  });

  it("rejects immediately when the external signal is pre-aborted", async () => {
    const spy = mockAbortAwareFetch();
    const ac = new AbortController();
    ac.abort();
    await expect(fetchWithTimeout("https://x.test/", { signal: ac.signal })).rejects.toThrow(
      /aborted/i,
    );
    expect(spy).toHaveBeenCalledTimes(1);
    const inner = spy.mock.calls[0]?.[1] as RequestInit | undefined;
    expect((inner?.signal as AbortSignal | undefined)?.aborted).toBe(true);
  });

  it("relays a mid-flight external abort onto the inner signal", async () => {
    let inner: AbortSignal | null | undefined;
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      inner = (init as RequestInit | undefined)?.signal;
      return new Promise<Response>((_resolve, reject) => {
        inner?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
      });
    });
    const ac = new AbortController();
    const p = fetchWithTimeout("https://x.test/", { signal: ac.signal });
    ac.abort();
    await expect(p).rejects.toThrow(/aborted/i);
    expect(inner?.aborted).toBe(true);
  });

  it("removes the abort listener from the external signal after success", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    const ac = new AbortController();
    const removeSpy = vi.spyOn(ac.signal, "removeEventListener");
    await fetchWithTimeout("https://x.test/", { signal: ac.signal });
    expect(removeSpy).toHaveBeenCalledWith("abort", expect.any(Function));
  });

  it("clears the pending timeout once the request succeeds", async () => {
    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("ok"));
    await fetchWithTimeout("https://x.test/");
    expect(clearTimeoutSpy).toHaveBeenCalled();
  });
});

describe("fetchJsonWithTimeout", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("owns one AbortController through headers and complete body decoding", async () => {
    let observed: AbortSignal | null | undefined;
    let bodyConsumed = false;
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      observed = (init as RequestInit | undefined)?.signal;
      return new Promise<Response>((resolve) => {
        const handle = (): void => {
          resolve(new Response('{"ok":1}'));
        };
        setTimeout(() => {
          bodyConsumed = true;
          handle();
        }, 10);
      });
    });
    const p = fetchJsonWithTimeout("https://x.test/", { headers: { accept: "application/json" } });
    await vi.advanceTimersByTimeAsync(10);
    const body = await p;
    expect(body).toEqual({ ok: 1 });
    expect(bodyConsumed).toBe(true);
    expect(observed?.aborted).toBe(false);
  });

  it("aborts a stalled body decode under the timeout and surfaces FetchTimeoutError", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      const sig = (init as RequestInit | undefined)?.signal;
      return new Promise<Response>((_resolve, reject) => {
        sig?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
      });
    });
    const p = fetchJsonWithTimeout("https://x.test/", { timeout: 500 });
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(500);
    await expect(p).rejects.toBeInstanceOf(FetchTimeoutError);
  });

  it("cancels stalled body-stream consumption inside res.json() past the timeout", async () => {
    let observed: AbortSignal | null | undefined;
    let streamErrored = false;
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      const sig = (init as RequestInit | undefined)?.signal;
      observed = sig;
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new TextEncoder().encode('{"ok":'));
          sig?.addEventListener("abort", () => {
            streamErrored = true;
            controller.error(new DOMException("aborted", "AbortError"));
          });
        },
      });
      return Promise.resolve(
        new Response(stream, {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      );
    });
    const p = fetchJsonWithTimeout("https://x.test/", { timeout: 500 });
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(500);
    await expect(p).rejects.toBeInstanceOf(FetchTimeoutError);
    expect(streamErrored).toBe(true);
    expect(observed?.aborted).toBe(true);
  });

  it("classifies a non-ok HTTP response as FetchHttpError without leaking the URL", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("nope", { status: 503 }));
    await expect(fetchJsonWithTimeout("https://secret.test/path")).rejects.toThrow(/http 503/);
    await expect(fetchJsonWithTimeout("https://secret.test/path")).rejects.toThrow(FetchHttpError);
  });
});

describe("ReadFailure classification", () => {
  it("classifies timeout, http, malformed, and transport distinctively", () => {
    expect(classifyApiFailure(new FetchTimeoutError()).kind).toBe("timeout");
    const http = classifyApiFailure(new FetchHttpError(502));
    expect(http.kind).toBe("http");
    expect(http.status).toBe(502);
    expect(classifyApiFailure(new MalformedDataError()).kind).toBe("malformed");
    expect(classifyApiFailure(new Error("network")).kind).toBe("transport");
    expect(classifyApiFailure(new DOMException("x", "AbortError")).kind).toBe("transport");
  });

  it("classifies any worker-tier error as worker", () => {
    expect(classifyWorkerFailure(new Error("engine not ready")).kind).toBe("worker");
  });

  it("ReadChainError retains both tier causes", () => {
    const workerFailure = { kind: "worker" as const };
    const apiFailure = { kind: "timeout" as const };
    const err = new ReadChainError("unavailable", workerFailure, apiFailure);
    expect(err.workerFailure).toEqual(workerFailure);
    expect(err.apiFailure).toEqual(apiFailure);
    expect(err.message).toBe("unavailable");
  });
});
