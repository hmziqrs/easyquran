import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import type { ResolvedManifest } from "$lib/quran/manifest";
import type { WorkerOutbound, WorkerRequest } from "$lib/quran/protocol";
import { QURAN_DATA } from "$lib/server/quran-data";

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
  emit(type: "error" | "messageerror", payload: unknown): void;
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

function respondReadSurah(fake: FakeWorker, result: unknown): void {
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

describe("withTranslationFallback via readSurah", () => {
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
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1);
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
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1);
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
      (m): m is Extract<WorkerRequest, { type: "ensureTranslation" }> => m.type === "ensureTranslation",
    )!;
    expect(ensure).toBeDefined();
    fake.emit("message", { id: ensure.id, ok: true, result: null });
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(0);
    respondHasTranslation(fake, true);
    await vi.advanceTimersByTimeAsync(0);
    respondReadSurah(fake, { cached: true });
    await expect(p).resolves.toBe(DECODED_SURAH);
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
    respondHasTranslation(fake, false);
    await expect(p).rejects.toThrow(/translation surah unavailable: en\.pickthall\/1/);
    try {
      await p;
    } catch (err) {
      expect((err as Error).cause).toBeInstanceOf(Error);
      expect(String((err as Error).cause)).toMatch(/api down/);
    }
  });
});

describe("withTranslationFallback without a worker", () => {
  it("wraps an API failure with the chained cause when worker is down", async () => {
    apiReads.readSurah.mockRejectedValue(new Error("api down"));
    const p = quranWorker.readSurah(1, TRANSLATION);
    await expect(p).rejects.toThrow(/translation surah unavailable/);
    try {
      await p;
    } catch (err) {
      expect((err as Error).cause).toBeInstanceOf(Error);
      expect(String((err as Error).cause)).toMatch(/api down/);
    }
    expect(apiReads.readSurah).toHaveBeenCalledWith(TRANSLATION, 1);
  });

  it("rejects without calling the API when both worker and apiBase are unavailable", async () => {
    siteConfig.apiBase = "";
    await expect(quranWorker.readSurah(1, TRANSLATION)).rejects.toThrow(
      /translation surah unavailable/,
    );
    expect(apiReads.readSurah).not.toHaveBeenCalled();
  });
});

describe("withTranslationFallback via readRange", () => {
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
    expect(apiReads.readRange).toHaveBeenCalledWith(TRANSLATION, 0, 1);
  });

  it("chains the underlying API error for readRange when every tier fails", async () => {
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
    respondHasTranslation(fake, false);
    await expect(p).rejects.toThrow(/translation range unavailable/);
    try {
      await p;
    } catch (err) {
      expect((err as Error).cause).toBeInstanceOf(Error);
    }
  });
});
