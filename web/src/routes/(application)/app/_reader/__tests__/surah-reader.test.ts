import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { mount, unmount } from "svelte";
import type { Component, ComponentProps } from "svelte";
import type { SurahLocalPageData } from "$lib/data/quran";

// ---- hoisted doubles -------------------------------------------------------
const {
  nav,
  workerStub,
  loadQuranDataStub,
  quranStore,
  invalidateAllSpy,
  gotoSpy,
  readerStub,
  mountStub,
} = vi.hoisted(() => ({
  nav: { state: {} },
  workerStub: {
    ready: true,
    readRange: vi.fn(),
    onStatus: vi.fn().mockReturnValue(() => {}),
  },
  loadQuranDataStub: vi.fn(),
  quranStore: { status: "idle" as string, error: null as string | null },
  invalidateAllSpy: vi.fn().mockResolvedValue(undefined),
  gotoSpy: vi.fn().mockResolvedValue(undefined),
  readerStub: {
    hasLastRead: false,
    lastRead: null,
    lastReadRef: "",
    markRead: vi.fn(),
    seedAyahs: vi.fn(),
  },
  mountStub: (() => {}) as unknown as Component,
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/navigation", () => ({
  beforeNavigate: () => {},
  goto: gotoSpy,
  invalidateAll: invalidateAllSpy,
  replaceState: () => {},
}));
vi.mock("$app/paths", () => ({ resolve: (p: string) => p }));
vi.mock("$app/state", () => ({ page: nav }));

vi.mock("$lib/data/quran-data-client", () => ({ loadQuranData: loadQuranDataStub }));
vi.mock("$lib/quran/worker-client", () => ({ quranWorker: workerStub }));
vi.mock("$lib/stores/quran.svelte", () => ({ quran: quranStore }));
vi.mock("$lib/stores/reader.svelte", () => ({
  reader: readerStub,
  ReaderMode: { Reading: "reading", Verse: "verse" },
}));

// child components as trivial stubs so mount never depends on their internals.
vi.mock("../ReaderHeader.svelte", () => ({ default: mountStub }));
vi.mock("../ReaderPageNav.svelte", () => ({ default: mountStub }));
vi.mock("../VerseRow.svelte", () => ({ default: mountStub }));

// ---- helpers ---------------------------------------------------------------
function flushMicrotasks(n = 12): Promise<void> {
  let p = Promise.resolve();
  for (let i = 0; i < n; i++) p = p.then(() => undefined);
  return p;
}

const SURAH = { num: 1, slug: "al-fatihah", name: "Al-Fatihah", arabic: "الفاتحة" };

function pageData(opts: { localPage?: number; ayahs?: number; pageCount?: number } = {}):
  // SurahLocalPageData with a minimal valid shape.
  Record<string, unknown> {
  const localPage = opts.localPage ?? 1;
  const count = opts.ayahs ?? 0;
  return {
    surah: SURAH,
    page: {
      surah: 1,
      localPage,
      globalPage: localPage,
      startGlobal: (localPage - 1) * 7 + 1,
      endGlobal: (localPage - 1) * 7 + 7,
      startAyah: 1,
      endAyah: 7,
      first: `1:1`,
      last: `1:7`,
    },
    pageCount: opts.pageCount ?? 3,
    ayahs: Array.from({ length: count }, (_, i) => ({
      key: `1:${i + 1}`,
      surah: 1,
      ayah: i + 1,
      globalIndex: (localPage - 1) * 7 + i + 1,
      text: `v${i + 1}`,
    })),
    normalization: {
      surah: 1,
      sourceId: "uthmani",
      script: "uthmani",
      sourceProfile: "p",
      packaging: "absent",
      openerKind: "none",
      openerText: null,
      openerEndScalar: 0,
      bodyStartScalar: 0,
    },
  };
}

// rAF queue: forward-fill / viewport callbacks queue here and flush on demand
// so page loads (and thus readRange) are driven deterministically.
const rafQueue: Array<() => void> = [];
function flushRaf(): void {
  const queue = rafQueue.splice(0);
  for (const cb of queue) {
    try {
      cb();
    } catch {
      /* swallow — rAF callbacks are non-fatal to the assertions */
    }
  }
}

function propsFor(initial: Record<string, unknown>): ComponentProps<typeof SurahReader> {
  return {
    initial: initial as unknown as SurahLocalPageData,
    previousPage: null,
    nextPage: null,
    previousSurah: null,
    nextSurah: null,
    anchorScrolling: false,
  };
}

import SurahReader from "../SurahReader.svelte";

let target: HTMLElement;

beforeEach(() => {
  vi.resetModules();
  target = document.createElement("div");
  document.body.appendChild(target);
  workerStub.readRange = vi.fn();
  workerStub.onStatus = vi.fn().mockReturnValue(() => {});
  workerStub.ready = true;
  loadQuranDataStub.mockReset();
  loadQuranDataStub.mockResolvedValue({
    surahLocalPage: () => ({ startGlobal: 8, endGlobal: 14 }),
    globalIndexOf: (s: number, a: number) => a,
    surahByNum: () => SURAH,
    surahLocalPageForAyah: () => ({ localPage: 1 }),
  });
  invalidateAllSpy.mockReset().mockResolvedValue(undefined);
  gotoSpy.mockReset().mockResolvedValue(undefined);
  quranStore.status = "idle";
  quranStore.error = null;
  readerStub.seedAyahs.mockReset();
  readerStub.markRead.mockReset();

  // happy-dom lacks ResizeObserver; the reader attaches one in two places.
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe(): void {}
      unobserve(): void {}
      disconnect(): void {}
    },
  );
  vi.stubGlobal(
    "BroadcastChannel",
    class {
      postMessage(): void {}
      close(): void {}
      addEventListener(): void {}
      removeEventListener(): void {}
    },
  );
  // Drive requestAnimationFrame synchronously so forward-fill / measurement
  // callbacks run within the test's microtask flushes.
  vi.stubGlobal(
    "requestAnimationFrame",
    vi.fn((cb: () => void) => {
      rafQueue.push(cb);
      return rafQueue.length;
    }),
  );
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  // Patch document.fonts for restoreHistory.
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { ready: Promise.resolve() },
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SurahReader W7 single-page empty recovery", () => {
  it("calls invalidateAll exactly once then falls back to W5 readRange (no goto)", async () => {
    const empty = pageData({ ayahs: 0, pageCount: 1 });
    // readRange resolves with content so the W5 fallback is observable.
    workerStub.readRange.mockResolvedValue({
      ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
      normalizations: [(empty as { normalization: unknown }).normalization],
    });

    mount(SurahReader, { target, props: propsFor(empty) });
    await flushMicrotasks(20);

    expect(invalidateAllSpy).toHaveBeenCalledTimes(1);
    expect(workerStub.readRange).toHaveBeenCalled();
    // Recovery never performs document navigation.
    expect(gotoSpy).not.toHaveBeenCalled();
  });

  it("does not re-enter retryInitialPage while one is already in flight", async () => {
    const empty = pageData({ ayahs: 0, pageCount: 1 });
    // Block invalidateAll so initialRetryInFlight stays true across triggers.
    let resolveInvalidate!: () => void;
    invalidateAllSpy.mockReturnValue(
      new Promise<void>((r) => (resolveInvalidate = r)),
    );

    mount(SurahReader, { target, props: propsFor(empty) });
    await flushMicrotasks();
    await flushMicrotasks();

    // Even after several flushes, only a single invalidateAll is in flight.
    expect(invalidateAllSpy).toHaveBeenCalledTimes(1);
    resolveInvalidate!();
    await flushMicrotasks();
  });
});

describe("SurahReader W7 distinct, clearable degradation state", () => {
  // Drive an adjacent-page load deterministically: forward-fill schedules an
  // rAF; flushing it triggers requestNextPage -> loadPage -> readRange, whose
  // onStatus callback is the SurahReader's degradation signal.
  async function driveAdjacentRead(): Promise<void> {
    await flushMicrotasks();
    flushRaf();
    await flushMicrotasks(20);
  }

  it("surfaces an API-only failure as the network-degraded message", async () => {
    const full = pageData({ ayahs: 7, pageCount: 3 });
    workerStub.readRange.mockImplementation(
      (_from: number, _to: number, _v?: unknown, _s?: unknown, onStatus?: (s: unknown) => void) => {
        onStatus?.({ servedBy: "local", apiFailure: { kind: "http", status: 503 } });
        return Promise.resolve({
          ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
          normalizations: [(full as { normalization: unknown }).normalization],
        });
      },
    );

    mount(SurahReader, { target, props: propsFor(full) });
    await driveAdjacentRead();

    expect(workerStub.readRange).toHaveBeenCalled();
    const region = target.querySelector('[role="status"]');
    // API-degraded message is the distinct "Network is slow…" copy.
    expect(region?.textContent ?? "").toMatch(/network is slow/i);
    expect(region?.textContent ?? "").not.toMatch(/local offline copy/i);
  });

  it("surfaces a worker-only failure as the local-offline message", async () => {
    const full = pageData({ ayahs: 7, pageCount: 3 });
    workerStub.readRange.mockImplementation(
      (_from: number, _to: number, _v?: unknown, _s?: unknown, onStatus?: (s: unknown) => void) => {
        onStatus?.({ servedBy: "api", workerFailure: { kind: "worker" } });
        return Promise.resolve({
          ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
          normalizations: [(full as { normalization: unknown }).normalization],
        });
      },
    );

    mount(SurahReader, { target, props: propsFor(full) });
    await driveAdjacentRead();

    expect(workerStub.readRange).toHaveBeenCalled();
    const region = target.querySelector('[role="status"]');
    // Worker-degraded message is the distinct "Local offline copy…" copy.
    expect(region?.textContent ?? "").toMatch(/local offline copy/i);
    expect(region?.textContent ?? "").not.toMatch(/network is slow/i);
  });

  it("retries a failed adjacent page in-page without document navigation", async () => {
    const full = pageData({ ayahs: 7, pageCount: 3 });
    // The adjacent page read rejects -> loadFailed/failedPage set.
    workerStub.readRange.mockRejectedValue(new Error("boom"));

    mount(SurahReader, { target, props: propsFor(full) });
    await driveAdjacentRead();

    // No document navigation on failure.
    expect(gotoSpy).not.toHaveBeenCalled();
    expect(invalidateAllSpy).not.toHaveBeenCalled();
    // Inline retry affordance is shown with the failed page number.
    const region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/couldn't be loaded/i);
    expect(target.querySelector('button[type="button"]')).not.toBeNull();
  });
});
