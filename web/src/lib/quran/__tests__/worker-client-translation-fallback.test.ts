import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import { ReadChainError } from "$lib/quran/fetch";
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
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- error/messageerror payloads follow the DOM Worker event contract (opaque event data); no test emits them
  emit(type: "error" | "messageerror", payload: unknown): void;
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- implementation signature must accept both the WorkerOutbound message payload and the opaque error payload above
  emit(type: string, payload: unknown): void {
    const evt = type === "message" ? { data: payload } : payload;
    this.listeners.get(type)?.forEach((fn) => fn(evt));
  }
}

const MANIFEST: ResolvedManifest = {
  scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "https://x/uthmani" }],
  source: "baked",
};

const TRANSLATION_NORMALIZATION = {
  surah: 1,
  sourceId: TRANSLATION,
  script: QuranScript.Translation,
  sourceProfile: "translation-profile",
  packaging: OpenerPackaging.Absent,
  openerKind: OpenerKind.None,
  openerText: null,
  openerEndScalar: 0,
  bodyStartScalar: 0,
} as const;

const DECODED_SURAH = {
  sourceId: TRANSLATION,
  script: QuranScript.Translation,
  verses: ["in the name", "of allah"],
  normalization: TRANSLATION_NORMALIZATION,
} as const;

const DECODED_RANGE = {
  ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 0, text: "in the name" }],
  normalizations: [TRANSLATION_NORMALIZATION],
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

function respondHasTranslation(fake: FakeWorker, value: boolean): void {
  const req = fake.posted.at(-1)!;
  expect(req.type).toBe("hasTranslation");
  fake.emit("message", { id: req.id, ok: true, result: value });
}

function respondReadSurah(
  fake: FakeWorker,
  result: { cached: boolean } | { downloaded: boolean },
): void {
  const req = fake.posted.at(-1)!;
  expect(req.type).toBe("readSurah");
  fake.emit("message", { id: req.id, ok: true, result });
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

describe("withSourceFallback via readSurah", () => {
  it("serves the cached worker result without touching the API when decode succeeds", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(DECODED_SURAH);
    const p = quranWorker.readSurah(1, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadSurah(fake, { cached: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).not.toHaveBeenCalled();
  });

  it("falls through to the API when the cached decode yields null", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(null);
    apiReads.readSurah.mockResolvedValue(DECODED_SURAH);
    const p = quranWorker.readSurah(1, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadSurah(fake, { cached: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1, undefined);
  });

  it("ensures the translation then returns the API result when not cached", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(DECODED_SURAH);
    apiReads.readSurah.mockResolvedValue(DECODED_SURAH);
    const p = quranWorker.readSurah(1, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    const ensure = fake.posted.at(-1)!;
    expect(ensure.type).toBe("ensureTranslation");
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1, undefined);
  });

  it("re-checks the cache after an API failure and returns the worker result", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(DECODED_SURAH);
    apiReads.readSurah.mockRejectedValueOnce(new Error("api down"));
    const p = quranWorker.readSurah(1, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    const ensure = fake.posted.find(
      (m): m is Extract<WorkerRequest, { type: "ensureTranslation" }> =>
        m.type === "ensureTranslation",
    )!;
    expect(ensure).toBeDefined();
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await vi.advanceTimersByTimeAsync(0);
    // API failed: the re-check forces the worker read (download-then-read for a cold
    // source) instead of only re-probing hasTranslation.
    respondReadSurah(fake, { downloaded: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).toHaveBeenCalledTimes(1);
  });

  it("serves a cold translation entirely from the worker when no read API is configured", async () => {
    const fake = await startReady();
    siteConfig.apiBase = "";
    wireMocks.decodeTranslationSurah.mockReturnValue(DECODED_SURAH);
    const p = quranWorker.readSurah(1, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    // The miss fires onMiss (ensureTranslation) and, with no apiBase, the forced
    // worker read posts immediately after it — no API leg in between.
    const ensure = fake.posted.find((m) => m.type === "ensureTranslation");
    expect(ensure).toBeDefined();
    fake.emit("message", { id: ensure!.id, ok: true, result: null });
    respondReadSurah(fake, { downloaded: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).not.toHaveBeenCalled();
  });

  it("throws the chained error message when every tier fails", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(null);
    apiReads.readSurah.mockRejectedValue(new Error("api down"));
    const p = quranWorker.readSurah(1, TRANSLATION);
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    const ensure = fake.posted.at(-1)!;
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await vi.advanceTimersByTimeAsync(0);
    const forced = fake.posted.at(-1)!;
    expect(forced.type).toBe("readSurah");
    fake.emit("message", { id: forced.id, ok: false, error: "translation fetch failed" });
    await expect(p).rejects.toThrow(/translation surah unavailable: en\.pickthall\/1/);
    try {
      await p;
    } catch (err) {
      expect(err).toBeInstanceOf(ReadChainError);
      // SAFETY: the preceding toBeInstanceOf(ReadChainError) check pins err's constructor before the cast
      expect((err as ReadChainError).apiFailure?.kind).toBe("transport");
    }
  });

  it("reports servedBy local without touching the API when the worker serves", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(DECODED_SURAH);
    const onStatus = vi.fn();
    const p = quranWorker.readSurah(1, TRANSLATION, onStatus);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadSurah(fake, { cached: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(apiReads.readSurah).not.toHaveBeenCalled();
    expect(onStatus).toHaveBeenCalledWith(expect.objectContaining({ servedBy: "local" }));
  });

  it("reports servedBy api with workerFailure when the API serves after a worker miss", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationSurah.mockReturnValue(null);
    apiReads.readSurah.mockResolvedValue(DECODED_SURAH);
    const onStatus = vi.fn();
    const p = quranWorker.readSurah(1, TRANSLATION, onStatus);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadSurah(fake, { cached: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
    expect(onStatus).toHaveBeenCalledWith(
      expect.objectContaining({
        servedBy: "api",
        workerFailure: { kind: "malformed" },
      }),
    );
  });
});

describe("withSourceFallback without a worker", () => {
  it("retains the typed API failure when worker is down", async () => {
    apiReads.readSurah.mockRejectedValue(new Error("api down"));
    const p = quranWorker.readSurah(1, TRANSLATION);
    await expect(p).rejects.toThrow(/translation surah unavailable/);
    try {
      await p;
    } catch (err) {
      expect(err).toBeInstanceOf(ReadChainError);
      // SAFETY: the preceding toBeInstanceOf(ReadChainError) check pins err's constructor before the cast
      expect((err as ReadChainError).apiFailure?.kind).toBe("transport");
      // The post-API-failure re-check forces a translation read even without a started
      // worker, so the absent engine is now recorded as the worker failure.
      // SAFETY: same ReadChainError instance confirmed two assertions above
      expect((err as ReadChainError).workerFailure).toEqual({ kind: "worker" });
    }
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1, undefined);
  });

  it("rejects without calling the API when both worker and apiBase are unavailable", async () => {
    siteConfig.apiBase = "";
    await expect(quranWorker.readSurah(1, TRANSLATION)).rejects.toThrow(
      /translation surah unavailable/,
    );
    expect(apiReads.readSurah).not.toHaveBeenCalled();
  });
});

describe("withSourceFallback via readRange", () => {
  it("serves the cached worker result for readRange without touching the API", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    const p = quranWorker.readRange(0, 1, undefined, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    const req = fake.posted.at(-1)!;
    expect(req.type).toBe("readRange");
    fake.emit("message", { id: req.id, ok: true, result: { cached: true } });
    await expect(p).resolves.toBe(DECODED_RANGE);
    expect(apiReads.readRange).not.toHaveBeenCalled();
  });

  it("falls back to the API for readRange when not cached", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationRange.mockReturnValue(DECODED_RANGE);
    apiReads.readRange.mockResolvedValue(DECODED_RANGE);
    const p = quranWorker.readRange(0, 1, undefined, TRANSLATION);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    const ensure = fake.posted.at(-1)!;
    expect(ensure.type).toBe("ensureTranslation");
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await expect(p).resolves.toBe(DECODED_RANGE);
    expect(apiReads.readRange).toHaveBeenCalledWith(TRANSLATION, 0, 1, undefined, undefined);
  });

  it("chains the underlying API failure for readRange when every tier fails", async () => {
    const fake = await startReady();
    wireMocks.decodeTranslationRange.mockReturnValue(null);
    apiReads.readRange.mockRejectedValue(new Error("api down"));
    const p = quranWorker.readRange(0, 1, undefined, TRANSLATION);
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, false);
    await vi.advanceTimersByTimeAsync(0);
    const ensure = fake.posted.at(-1)!;
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await vi.advanceTimersByTimeAsync(0);
    const forced = fake.posted.at(-1)!;
    expect(forced.type).toBe("readRange");
    fake.emit("message", { id: forced.id, ok: false, error: "translation fetch failed" });
    await expect(p).rejects.toThrow(/translation range unavailable/);
    try {
      await p;
    } catch (err) {
      expect(err).toBeInstanceOf(ReadChainError);
      // SAFETY: the preceding toBeInstanceOf(ReadChainError) check pins err's constructor before the cast
      expect((err as ReadChainError).apiFailure?.kind).toBe("transport");
    }
  });
});
