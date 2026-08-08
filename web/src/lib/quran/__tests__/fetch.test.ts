import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { FETCH_TIMEOUT_MS, fetchWithTimeout } from "$lib/quran/fetch";

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
        inner?.addEventListener("abort", () =>
          reject(new DOMException("aborted", "AbortError")),
        );
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
