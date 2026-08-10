import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import {
  createRangeReaderCoordinator,
  equalRangeKey,
  quranWorker,
  rangeRouteKey,
  type RangeDisplaySnapshot,
} from "$lib/quran/worker-client";
import type { Ayah, SurahLink, SurahNormalization } from "$lib/data/quran-types";

const { siteConfig, apiReads, wireMocks } = vi.hoisted(() => ({
  siteConfig: { apiBase: "https://api.test/quran" },
  apiReads: { readRange: vi.fn() },
  wireMocks: { decodeArabicRange: vi.fn() },
}));

vi.mock("$lib/config/site", () => ({ QURAN: siteConfig }));
vi.mock("$lib/quran/api-client", () => ({
  quranApi: { readRange: apiReads.readRange },
}));
vi.mock("$lib/quran/wire", () => ({
  decodeQuranRangeText: wireMocks.decodeArabicRange,
  decodeTranslationRangeText: vi.fn(),
  decodeQuranSurahText: vi.fn(),
  decodeTranslationSurahText: vi.fn(),
  decodeSearchResponse: vi.fn(),
  unwrapEnvelope: vi.fn(),
}));

function ayah(surah: number, n: number, globalIndex: number): Ayah {
  return { key: `${surah}:${n}`, surah, ayah: n, globalIndex, text: `v-${surah}-${n}` };
}

function norm(surah: number, sourceId: string): SurahNormalization {
  return {
    surah,
    sourceId,
    script: "translation",
    sourceProfile: "p",
    packaging: "absent",
    openerKind: "none",
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
}

function surah(num: number): SurahLink {
  return { num, slug: `s${num}`, name: `Surah ${num}`, arabic: "ع" };
}

function snapshot(
  ayahs: Ayah[],
  normalizations: SurahNormalization[],
  surahs: SurahLink[],
): RangeDisplaySnapshot {
  return { ayahs, normalizations, surahs };
}

const FULL_A = snapshot([ayah(1, 1, 0), ayah(1, 2, 1)], [norm(1, "x")], [surah(1)]);
const FULL_B = snapshot([ayah(2, 1, 5)], [norm(2, "x")], [surah(2)]);
const DEGRADED = snapshot([], [], []);

describe("range reader coordinator — post-paint swap decision logic", () => {
  it("a complete first paint does not trigger a duplicate client read", () => {
    const c = createRangeReaderCoordinator();
    const key = rangeRouteKey(null, "juz", 30);
    expect(c.installServer(key, FULL_A).read).toBe(false);
    expect(c.installServer(key, FULL_A).read).toBe(false);
    expect(c.isDegraded()).toBe(false);
    expect(c.canRetry()).toBeNull();
  });

  it("an empty translated SSR first paint recovers complete ayahs/normalizations/surah metadata", async () => {
    const c = createRangeReaderCoordinator();
    const key = rangeRouteKey("en.sahih", "page", 1);
    expect(c.installServer(key, DEGRADED).read).toBe(true);
    expect(c.isDegraded()).toBe(true);
    expect(c.canRetry()).toEqual(key);

    const applied = c.applyClientResult(key, FULL_A).applied;
    expect(applied).toBe(true);
    expect(c.isDegraded()).toBe(false);
    expect(c.canRetry()).toBeNull();

    const out = c.currentSnapshot();
    expect(out.ayahs).toHaveLength(2);
    expect(out.normalizations).toHaveLength(1);
    expect(out.surahs).toEqual(FULL_A.surahs);
  });

  it("never combines client ayahs with server normalization/metadata (atomic snapshot)", () => {
    const c = createRangeReaderCoordinator();
    const key = rangeRouteKey("en.sahih", "juz", 1);
    const serverNorm = norm(1, "server");
    c.installServer(key, snapshot([], [], []));

    const clientNorm = norm(1, "client");
    c.applyClientResult(key, snapshot([ayah(1, 1, 0)], [clientNorm], [surah(1)]));

    const out = c.currentSnapshot();
    expect(out.normalizations[0]?.sourceId).toBe("client");
    expect(out.normalizations[0]?.sourceId).not.toBe("server");
    expect(out.ayahs[0]?.text).toBe("v-1-1");
    expect(out.surahs[0]).toEqual(surah(1));
    void serverNorm;
  });

  it("Arabic total failure keeps the matching server snapshot (no blank, no restore)", () => {
    const c = createRangeReaderCoordinator();
    const a = rangeRouteKey(null, "juz", 30);
    const b = rangeRouteKey(null, "juz", 29);
    expect(c.installServer(a, FULL_A).read).toBe(false);
    expect(c.installServer(b, FULL_B).read).toBe(true);

    c.markFailed(b, { kind: "worker" });
    expect(c.isDegraded()).toBe(false);
    expect(c.currentSnapshot()).toBe(FULL_B);
    expect(c.lastFailure()?.kind).toBe("worker");
  });

  it("translation total failure keeps the degraded server snapshot with typed degradation", () => {
    const c = createRangeReaderCoordinator();
    const key = rangeRouteKey("en.sahih", "page", 5);
    c.installServer(key, DEGRADED);
    c.markFailed(key, { kind: "http", status: 503 });
    expect(c.isDegraded()).toBe(true);
    expect(c.currentSnapshot().ayahs).toHaveLength(0);
    expect(c.lastFailure()).toEqual({ kind: "http", status: 503 });
  });

  it("a delayed old result after index navigation is discarded and does not mix routes", () => {
    const c = createRangeReaderCoordinator();
    const key1 = rangeRouteKey("en.sahih", "juz", 30);
    const key2 = rangeRouteKey("en.sahih", "juz", 29);
    c.installServer(key1, DEGRADED);
    c.installServer(key2, DEGRADED);

    expect(c.applyClientResult(key1, FULL_A).applied).toBe(false);
    expect(c.currentSnapshot()).toBe(DEGRADED);

    expect(c.applyClientResult(key2, FULL_B).applied).toBe(true);
    expect(c.currentSnapshot()).toBe(FULL_B);
  });

  it("a delayed old result after source navigation is discarded across the source boundary", () => {
    const c = createRangeReaderCoordinator();
    const arabic = rangeRouteKey(null, "juz", 30);
    const translated = rangeRouteKey("en.sahih", "juz", 30);
    c.installServer(arabic, FULL_A);
    c.installServer(translated, DEGRADED);

    expect(c.applyClientResult(arabic, FULL_A).applied).toBe(false);
    expect(c.currentSnapshot()).toBe(DEGRADED);
  });

  it("worker readiness retries only the current degraded key", () => {
    const c = createRangeReaderCoordinator();
    const k1 = rangeRouteKey("en.sahih", "page", 1);
    const k2 = rangeRouteKey("en.sahih", "page", 2);
    c.installServer(k1, DEGRADED);
    expect(c.canRetry()).toEqual(k1);

    c.installServer(k2, DEGRADED);
    expect(c.canRetry()).toEqual(k2);
    expect(c.canRetry()).not.toEqual(k1);

    c.applyClientResult(k2, FULL_B);
    expect(c.canRetry()).toBeNull();
  });

  it("a later navigation runs the chain even when the new server paint is complete", () => {
    const c = createRangeReaderCoordinator();
    const a = rangeRouteKey(null, "juz", 30);
    const b = rangeRouteKey(null, "juz", 29);
    c.installServer(a, FULL_A);
    expect(c.installServer(b, FULL_B).read).toBe(true);
  });

  it("juz 30 round-trips through the coordinator on arabic and translation paths", () => {
    const arabic = rangeRouteKey(null, "juz", 30);
    const translated = rangeRouteKey("ms.basmeih", "juz", 30);
    expect(equalRangeKey(arabic, translated)).toBe(false);
    expect(arabic.sourceId).toBeNull();
    expect(translated.sourceId).toBe("ms.basmeih");

    const c = createRangeReaderCoordinator();
    expect(c.installServer(arabic, FULL_A).read).toBe(false);
    expect(c.installServer(translated, DEGRADED).read).toBe(true);
    expect(c.applyClientResult(translated, FULL_B).applied).toBe(true);
  });

  it("no path blanks: currentSnapshot always reflects the installed server snapshot until a matching client result", () => {
    const c = createRangeReaderCoordinator();
    expect(c.currentSnapshot().ayahs).toHaveLength(0);
    const key = rangeRouteKey("en.sahih", "page", 3);
    c.installServer(key, FULL_A);
    expect(c.currentSnapshot()).toBe(FULL_A);
    c.markFailed(key, { kind: "transport" });
    expect(c.currentSnapshot()).toBe(FULL_A);
  });
});

describe("range reader readRange reaches the API when the worker is unavailable (arabic path)", () => {
  beforeEach(() => {
    apiReads.readRange.mockReset();
    wireMocks.decodeArabicRange.mockReset();
    siteConfig.apiBase = "https://api.test/quran";
  });

  afterEach(() => {
    quranWorker.dispose();
  });

  it("falls through to quranApi.readRange when the worker is not started", async () => {
    wireMocks.decodeArabicRange.mockReturnValue(FULL_A);
    apiReads.readRange.mockResolvedValue(FULL_A);
    const result = await quranWorker.readRange(0, 1);
    expect(result).toBe(FULL_A);
    expect(apiReads.readRange).toHaveBeenCalled();
  });
});
