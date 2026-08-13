import { LOCAL_HEDGE_BUDGET_MS, ReadChainError } from "$lib/quran/fetch";
import type { ResolvedManifest } from "$lib/quran/manifest";
import type { WorkerOutbound, WorkerRequest } from "$lib/quran/protocol";
import { QURAN_DATA } from "$lib/server/quran-data";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { siteConfig, apiReads, wireMocks } = vi.hoisted(() => ({
  siteConfig: { apiBase: "https://api.test/quran" },
  apiReads: {
    readSurah: vi.fn(),
    readRange: vi.fn(),
  },
  wireMocks: {
    decodeTranslationSurah: vi.fn(),
    decodeTranslationRange: vi.fn(),
    decodeArabicSurah: vi.fn(),
    decodeArabicRange: vi.fn(),
    decodeSearch: vi.fn(),
    unwrap: vi.fn(),
  },
}));

vi.mock("$lib/config/site", () => ({ QURAN: siteConfig }));
vi.mock("$lib/quran/api-client", () => ({
  quranApi: { readSurah: apiReads.readSurah, readRange: apiReads.readRange },
}));
vi.mock("$lib/quran/wire", () => ({
  decodeTranslationSurahText: wireMocks.decodeTranslationSurah,
  decodeTranslationRangeText: wireMocks.decodeTranslationRange,
  decodeQuranSurahText: wireMocks.decodeArabicSurah,
  decodeQuranRangeText: wireMocks.decodeArabicRange,
  decodeSearchResponse: wireMocks.decodeSearch,
  unwrapEnvelope: wireMocks.unwrap,
}));

import { quranWorker } from "$lib/quran/worker-client";

const TRANSLATION = "en.pickthall";
const HEDGE = { hedgeAfterMs: LOCAL_HEDGE_BUDGET_MS };

// eslint-disable-next-line anti-slop/no-unknown-parameters -- mirrors the DOM Worker addEventListener callback contract; quranWorker registers its own opaque message listener, so the event shape stays unknown at this fake boundary
type Listener = (e: unknown) => void;

class FakeWorker {
  static last: FakeWorker | null = null;
  readonly listeners = new Map<string, Set<Listener>>();
  readonly posted: WorkerRequest[] = [];
  terminated = false;

  constructor() {
    FakeWorker.last = this;
  }
  addEventListener(type: string, fn: Listener): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(fn);
  }
  removeEventListener(type: string, fn: Listener): void {
    this.listeners.get(type)?.delete(fn);
  }
  postMessage(msg: WorkerRequest): void {
    this.posted.push(msg);
  }
  terminate(): void {
    this.terminated = true;
  }
  emit(type: "message", data: WorkerOutbound): void;
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- implementation signature must accept the WorkerOutbound payload and the opaque DOM error payload alike
  emit(type: string, payload: unknown): void {
    const evt = type === "message" ? { data: payload } : payload;
    this.listeners.get(type)?.forEach((fn) => fn(evt));
  }
}

const MANIFEST: ResolvedManifest = {
  scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "https://x/uthmani" }],
  source: "baked",
};

const DECODED_RANGE = {
  ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "local" }],
  normalizations: [],
} as const;

const API_RANGE = {
  ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "api" }],
  normalizations: [],
} as const;

async function startReady(): Promise<FakeWorker> {
  const started = quranWorker.start(MANIFEST, QURAN_DATA.coordinates);
  const fake = FakeWorker.last!;
  const init = fake.posted.find(
    (m): m is Extract<WorkerRequest, { type: "init" }> => m.type === "init",
  )!;
  fake.emit("message", { id: init.id, ok: true, result: null });
  await started;
  return fake;
}

function lastOfType(fake: FakeWorker, type: WorkerRequest["type"]): WorkerRequest | undefined {
  return fake.posted.filter((m) => m.type === type).at(-1);
}

function respondHasTranslation(fake: FakeWorker, value: boolean): void {
  const req = lastOfType(fake, "hasTranslation")!;
  fake.emit("message", { id: req.id, ok: true, result: value });
}

function respondReadRange(fake: FakeWorker): void {
  const req = lastOfType(fake, "readRange")!;
  fake.emit("message", { id: req.id, ok: true, result: { rows: [] } });
}

beforeEach(() => {
  vi.useFakeTimers();
  FakeWorker.last = null;
  vi.stubGlobal("Worker", FakeWorker);
  apiReads.readSurah.mockReset();
  apiReads.readRange.mockReset();
  wireMocks.decodeTranslationSurah.mockReset();
  wireMocks.decodeTranslationRange.mockReset();
  siteConfig.apiBase = "https://api.test/quran";
});

afterEach(() => {
  const fake = FakeWorker.last;
  if (fake) {
    for (const msg of fake.posted) {
      if ("id" in msg) fake.emit("message", { id: msg.id, ok: true, result: null });
    }
  }
  quranWorker.dispose();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("hedged translation reads", () => {
  it("serves from the API when the worker stays silent past the hedge budget", async () => {
    await startReady();
    apiReads.readRange.mockResolvedValue(API_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);

    // Worker never answers hasTranslation: it is busy staging an artifact.
    await vi.advanceTimersByTimeAsync(0);
    expect(apiReads.readRange).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    await expect(p).resolves.toBe(API_RANGE);
    expect(apiReads.readRange).toHaveBeenCalledTimes(1);
    expect(apiReads.readRange.mock.calls[0]?.[3]).toBeInstanceOf(AbortSignal);
  });

  it("reports the tier as api when the hedge wins", async () => {
    await startReady();
    apiReads.readRange.mockResolvedValue(API_RANGE);
    const onStatus = vi.fn();
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, onStatus, HEDGE);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    await p;
    expect(onStatus).toHaveBeenCalledTimes(1);
    expect(onStatus).toHaveBeenCalledWith({ servedBy: "api", workerFailure: undefined });
  });

  it("keeps the local result and aborts the API leg when the worker answers in time", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).resolves.toBe(DECODED_RANGE);
    expect(apiReads.readRange).not.toHaveBeenCalled();
  });

  it("fires the API immediately on a known local miss, without waiting out the budget", async () => {
    const fake = await startReady();
    apiReads.readRange.mockResolvedValue(API_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    expect(apiReads.readRange).toHaveBeenCalledTimes(1);
    expect(lastOfType(fake, "ensureTranslation")).toBeDefined();
    await expect(p).resolves.toBe(API_RANGE);
  });

  it("still starts the OPFS download when the API wins the race", async () => {
    const fake = await startReady();
    apiReads.readRange.mockResolvedValue(API_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    await expect(p).resolves.toBe(API_RANGE);

    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    expect(lastOfType(fake, "ensureTranslation")).toBeDefined();
  });

  it("falls back to the late local result when the API leg fails", async () => {
    const fake = await startReady();
    apiReads.readRange.mockRejectedValue(new Error("api down"));
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const onStatus = vi.fn();
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, onStatus, HEDGE);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).resolves.toBe(DECODED_RANGE);
    expect(onStatus).toHaveBeenCalledWith({ servedBy: "local" });
  });

  it("raises a typed ReadChainError when both legs fail", async () => {
    const fake = await startReady();
    apiReads.readRange.mockRejectedValue(new Error("api down"));
    wireMocks.decodeTranslationRange.mockReturnValue(null);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    // Post-failure recheck.
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).rejects.toBeInstanceOf(ReadChainError);
    try {
      await p;
    } catch (err) {
      expect(err).toBeInstanceOf(ReadChainError);
      // SAFETY: the preceding toBeInstanceOf(ReadChainError) check pins err's constructor before the cast
      expect((err as ReadChainError).apiFailure).toEqual({ kind: "transport" });
      // SAFETY: same ReadChainError instance confirmed two assertions above
      expect((err as ReadChainError).workerFailure).toEqual({ kind: "malformed" });
    }
  });

  it("stays sequential when no api base is configured", async () => {
    siteConfig.apiBase = "";
    const fake = await startReady();
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, HEDGE);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    expect(apiReads.readRange).not.toHaveBeenCalled();
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).resolves.toBe(DECODED_RANGE);
  });

  it("never hedges a read that omits the option", async () => {
    const fake = await startReady();
    apiReads.readRange.mockResolvedValue(API_RANGE);
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION);
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS * 4);
    expect(apiReads.readRange).not.toHaveBeenCalled();
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).resolves.toBe(DECODED_RANGE);
  });

  it("skips the API leg when the caller has already aborted", async () => {
    const fake = await startReady();
    apiReads.readRange.mockImplementation((_id: string, _f: number, _t: number, signal?: AbortSignal) => {
      if (signal?.aborted) return Promise.reject(new Error("aborted"));
      return Promise.resolve(API_RANGE);
    });
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const controller = new AbortController();
    controller.abort();
    const p = quranWorker.readRange(1, 1, undefined, TRANSLATION, undefined, {
      ...HEDGE,
      signal: controller.signal,
    });
    await vi.advanceTimersByTimeAsync(LOCAL_HEDGE_BUDGET_MS);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadRange(fake);
    await expect(p).resolves.toBe(DECODED_RANGE);
  });
});
