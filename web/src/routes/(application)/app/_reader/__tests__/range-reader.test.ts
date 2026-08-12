import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { mount, unmount } from "svelte";
import type { Component } from "svelte";
import {
  createRangeReaderCoordinator,
  equalRangeKey,
  quranWorker,
  rangeRouteKey,
  type RangeDisplaySnapshot,
} from "$lib/quran/worker-client";
import type {
  Ayah,
  RangePageData,
  SurahLink,
  SurahNormalization,
} from "$lib/data/quran-types";
import RangeReader from "../RangeReader.svelte";
import RangeReaderHost from "./RangeReaderHost.svelte";

const { siteConfig, apiReads, wireMocks } = vi.hoisted(() => ({
  siteConfig: { apiBase: "https://api.test/quran" },
  apiReads: { readRange: vi.fn() },
  wireMocks: { decodeArabicRange: vi.fn() },
}));

// Mount-test doubles. Kept separate from the coordinator doubles above so the
// pure-logic describes stay self-contained.
const { nav, loadQuranDataStub, gotoSpy, verseRowStub, tooltipStub } = vi.hoisted(() => ({
  nav: { params: { lang: "en", translator: "sahih" } },
  loadQuranDataStub: vi.fn(),
  gotoSpy: vi.fn().mockResolvedValue(undefined),
  verseRowStub: (() => {}) as unknown as Component,
  tooltipStub: (() => {}) as unknown as Component,
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

// Mount-test wiring: RangeReader reaches $app/*, loadQuranData, track-view, and
// the VerseRow/Tooltip children. Only the async boundaries are stubbed — the
// real coordinator, real quranWorker (which falls through to the mocked
// quranApi.readRange when the worker is not started), and real view helpers
// (groupRangeAyahs/bodyText) run unmodified.
vi.mock("$app/navigation", () => ({ goto: gotoSpy }));
vi.mock("$app/paths", () => ({ resolve: (p: string) => p, base: "" }));
vi.mock("$app/state", () => ({ page: nav }));
vi.mock("$lib/data/quran-data-client", () => ({ loadQuranData: loadQuranDataStub }));
vi.mock("$lib/quran/track-view.svelte", () => ({ trackReaderView: () => {} }));
vi.mock("../VerseRow.svelte", () => ({ default: verseRowStub }));
vi.mock("$lib/components/ui/tooltip", () => ({ TooltipProvider: tooltipStub }));

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

// ---------------------------------------------------------------------------
// Mount tests: verify the actual post-paint swap wiring inside RangeReader
// ($effect.pre -> coordinator.installServer -> runClientRead -> displayed
// reassignment) and the surah-metadata derivation, neither of which the
// coordinator-only describes above can see. The real quranWorker falls through
// to the mocked quranApi.readRange (worker not started), so the API stub is the
// client-read boundary.
// ---------------------------------------------------------------------------

function flushMicrotasks(n = 12): Promise<void> {
  let p = Promise.resolve();
  for (let i = 0; i < n; i++) p = p.then(() => undefined);
  return p;
}

// Minimal RangePageData (the `data` prop). Defaults model a degraded
// translation SSR paint: empty ayahs/normalizations/surahs -> the coordinator
// returns read=true, so $effect.pre kicks runClientRead.
function rangePage(opts: {
  kind?: "juz" | "page";
  index: number;
  startGlobal: number;
  endGlobal: number;
  ayahs?: Ayah[];
  normalizations?: SurahNormalization[];
  surahs?: SurahLink[];
}): RangePageData {
  const kind = opts.kind ?? "juz";
  const ayahs = opts.ayahs ?? [];
  return {
    kind,
    index: opts.index,
    label: `${kind === "juz" ? "Juz" : "Page"} ${opts.index}`,
    startGlobal: opts.startGlobal,
    endGlobal: opts.endGlobal,
    first: ayahs[0]?.key ?? "1:1",
    last: ayahs[ayahs.length - 1]?.key ?? "1:1",
    ayahs,
    normalizations: opts.normalizations ?? [],
    surahs: opts.surahs ?? [],
  };
}

// Surah catalogue returned by loadQuranData(). runClientRead derives the
// displayed surah metadata from this — names here are intentionally distinct
// from any server paint so a rendered name proves the client derivation ran.
function seedQuranData(): void {
  loadQuranDataStub.mockResolvedValue({
    surahs: [
      { num: 1, slug: "al-fatihah", name: "Al-Fatihah", arabic: "الفاتحة" },
      { num: 2, slug: "al-baqarah", name: "Al-Baqarah", arabic: "البقرة" },
    ],
    globalIndexOf: (_s: number, a: number) => a,
  });
}

describe("RangeReader mount — post-paint swap wiring + surah-metadata derivation", () => {
  let target: HTMLElement;
  let mounted: ReturnType<typeof mount> | undefined;

  beforeEach(() => {
    apiReads.readRange.mockReset();
    wireMocks.decodeArabicRange.mockReset();
    siteConfig.apiBase = "https://api.test/quran";
    nav.params = { lang: "en", translator: "sahih" };
    loadQuranDataStub.mockReset();
    seedQuranData();
    gotoSpy.mockReset().mockResolvedValue(undefined);
    target = document.createElement("div");
    document.body.appendChild(target);
  });

  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = undefined;
    quranWorker.dispose();
    if (target.parentNode) target.parentNode.removeChild(target);
  });

  it("runs the client read after a degraded SSR paint and swaps in derived ayahs + surah metadata", async () => {
    // Degraded translation SSR paint: no ayahs, no normalizations, no surahs.
    const degraded = rangePage({ index: 30, startGlobal: 5700, endGlobal: 5720 });
    // Client read recovers surah 1 content.
    apiReads.readRange.mockResolvedValue({
      ayahs: [ayah(1, 1, 5700)],
      normalizations: [norm(1, "en.sahih")],
    });

    mounted = mount(RangeReader, { target, props: { data: degraded } });
    await flushMicrotasks(20);

    // Wiring: $effect.pre -> coordinator.installServer (read=true) ->
    // runClientRead reached the worker, which fell through to quranApi.readRange.
    expect(apiReads.readRange).toHaveBeenCalled();
    // Surah metadata is derived in runClientRead from loadQuranData().surahs;
    // the server paint carried none, so the name can only come from the client.
    expect(target.textContent ?? "").toMatch(/Al-Fatihah/);
    // displayed reassignment: the degraded fallback is gone (displayed.ayahs > 0).
    expect(target.textContent ?? "").not.toMatch(/couldn't be loaded/i);
  });

  it("does not run a client read when the SSR first paint is already complete", async () => {
    // Arabic-like complete paint: ayahs + a matching normalization present.
    const complete = rangePage({
      index: 30,
      startGlobal: 5700,
      endGlobal: 5720,
      ayahs: [ayah(1, 1, 5700)],
      normalizations: [norm(1, "uthmani")],
      surahs: [surah(1)],
    });

    mounted = mount(RangeReader, { target, props: { data: complete } });
    await flushMicrotasks(20);

    // installServer returned read=false (non-degraded first paint), so the
    // post-paint swap chain never reaches the worker/API.
    expect(apiReads.readRange).not.toHaveBeenCalled();
    // The complete server paint renders directly: the surah name comes from the
    // server paint (surah(1) -> "Surah 1"), not the loadQuranData catalogue
    // ("Al-Fatihah"), confirming runClientRead never derived client metadata.
    expect(target.textContent ?? "").toMatch(/Surah 1/);
    expect(target.textContent ?? "").not.toMatch(/Al-Fatihah/);
  });
});

describe("RangeReader mount — atomic snapshot guarded by route key on a living instance", () => {
  let target: HTMLElement;
  let mounted: ReturnType<typeof mount> | undefined;

  beforeEach(() => {
    apiReads.readRange.mockReset();
    wireMocks.decodeArabicRange.mockReset();
    siteConfig.apiBase = "https://api.test/quran";
    nav.params = { lang: "en", translator: "sahih" };
    loadQuranDataStub.mockReset();
    seedQuranData();
    gotoSpy.mockReset().mockResolvedValue(undefined);
    target = document.createElement("div");
    document.body.appendChild(target);
  });

  afterEach(() => {
    if (mounted) unmount(mounted);
    mounted = undefined;
    quranWorker.dispose();
    if (target.parentNode) target.parentNode.removeChild(target);
  });

  // Svelte 5 removed imperative $set on mounted instances, so a host component
  // (RangeReaderHost) drives the reactive `data` prop update on the LIVING
  // RangeReader. The coordinator is created once at init and persists across
  // the navigation, which is what makes the route-key guard meaningful here —
  // a remount would spawn a fresh coordinator and could not test the discard.
  it("discards a delayed result from the previous route and keeps the current route's snapshot", async () => {
    // Deferred API promises keyed by the range's start global (juz 30 vs juz 29
    // have distinct ranges), so each client read blocks until we release it.
    const pending = new Map<number, (v: unknown) => void>();
    apiReads.readRange.mockImplementation((_reader: unknown, from: number) => {
      return new Promise((r) => {
        pending.set(from, r as (v: unknown) => void);
      });
    });

    const juz30 = rangePage({ index: 30, startGlobal: 5700, endGlobal: 5720 });
    const juz29 = rangePage({ index: 29, startGlobal: 5500, endGlobal: 5520 });

    let navigate!: (next: RangePageData) => void;
    mounted = mount(RangeReaderHost, {
      target,
      props: { initial: juz30, expose: (fn) => (navigate = fn) },
    });
    await flushMicrotasks(20);
    // The juz 30 client read is in flight; its result has not been applied.
    expect(pending.has(5700)).toBe(true);

    // Navigate to juz 29 on the living instance (same coordinator).
    navigate(juz29);
    await flushMicrotasks(20);
    expect(pending.has(5500)).toBe(true);

    // Release the CURRENT route's read first: surah 2 content is applied.
    pending.get(5500)!({ ayahs: [ayah(2, 1, 5500)], normalizations: [norm(2, "en.sahih")] });
    await flushMicrotasks(20);
    expect(target.textContent ?? "").toMatch(/Al-Baqarah/);

    // Now release the STALE juz 30 read (surah 1). The route-key guard must
    // discard it: coord.applyClientResult returns applied=false, so displayed
    // is not reassigned and the current route's snapshot is preserved.
    pending.get(5700)!({ ayahs: [ayah(1, 1, 5700)], normalizations: [norm(1, "en.sahih")] });
    await flushMicrotasks(20);

    expect(target.textContent ?? "").toMatch(/Al-Baqarah/);
    expect(target.textContent ?? "").not.toMatch(/Al-Fatihah/);
  });
});
