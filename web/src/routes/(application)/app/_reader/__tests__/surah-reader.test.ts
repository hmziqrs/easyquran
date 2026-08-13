import type { SurahLocalPageData } from "$lib/data/quran";
import type { QuranReaderSource } from "$lib/data/quran-types";
import type { ReadTierStatus } from "$lib/quran/fetch";
import type { AyahCoordinateValidator } from "$lib/quran/wire";
import { mount, unmount } from "svelte";
import type { ComponentProps } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

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
  // SAFETY: the double mirrors the real store contract; beforeEach reassigns status to arbitrary status strings and error to string | null, and both seed values are members of those unions.
  quranStore: { status: "idle" as string, error: null as string | null },
  invalidateAllSpy: vi.fn().mockResolvedValue(undefined),
  gotoSpy: vi.fn().mockResolvedValue(undefined),
  readerStub: {
    hasLastRead: false,
    // SAFETY: tests below reassign lastRead to { num, n, sourceId } objects or null; null is a member of that union.
    lastRead: null as { num: number; n: number; sourceId?: string } | null,
    lastReadRef: "",
    // SAFETY: null is a member of the seeded union; no test in this file ever sets a concrete anchor.
    lastReadAnchor: null as { verseKey: string; localPage: number; ratio: number } | null,
    markRead: vi.fn(),
    setLastReadAnchor: vi.fn(),
    consumePendingAnchor: vi.fn(() => null),
    seedAyahs: vi.fn(),
  },
  mountStub: () => {},
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
vi.mock("$lib/quran/catalogue-store.svelte", () => ({
  catalogueStore: { translations: [] },
}));
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

// SurahLocalPageData with a minimal valid shape.
function pageData(
  opts: { localPage?: number; ayahs?: number; pageCount?: number } = {},
) {
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

function propsFor(initial: ReturnType<typeof pageData>): ComponentProps<typeof SurahReader> {
  return {
    // SAFETY: pageData() builds a minimal valid SurahLocalPageData; the only difference from the declared type is widened field literals (e.g. script: string vs union), so the assertion is a narrowing, not a shape change.
    initial: initial as SurahLocalPageData,
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
    globalIndexOf: (_s: number, a: number) => a,
    surahByNum: () => SURAH,
    surahLocalPageForAyah: () => ({ localPage: 1 }),
  });
  invalidateAllSpy.mockReset().mockResolvedValue(undefined);
  gotoSpy.mockReset().mockResolvedValue(undefined);
  quranStore.status = "idle";
  quranStore.error = null;
  readerStub.seedAyahs.mockReset();
  readerStub.markRead.mockReset();
  readerStub.hasLastRead = false;
  readerStub.lastRead = null;

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
      normalizations: [empty.normalization],
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
    invalidateAllSpy.mockReturnValue(new Promise<void>((r) => (resolveInvalidate = r)));

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
      (
        _from: number,
        _to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        onStatus?.({ servedBy: "local", apiFailure: { kind: "http", status: 503 } });
        return Promise.resolve({
          ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
          normalizations: [full.normalization],
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
      (
        _from: number,
        _to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        onStatus?.({ servedBy: "api", workerFailure: { kind: "worker" } });
        return Promise.resolve({
          ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
          normalizations: [full.normalization],
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

describe("SurahReader W7 degradation state lifecycle", () => {
  async function driveAdjacentRead(): Promise<void> {
    await flushMicrotasks();
    flushRaf();
    await flushMicrotasks(20);
  }

  // Re-arm forward-fill via a resize so the next page (requestNextPage) loads
  // deterministically without depending on happy-dom scroll geometry.
  function fireForwardFill(): void {
    window.dispatchEvent(new Event("resize"));
    flushRaf();
  }

  // Force an upward-scroll processScroll cycle so requestPreviousPage runs.
  // lastScrollY starts at 0 on mount; a negative scrollY yields direction -1.
  function scrollUp(): void {
    Object.defineProperty(window, "scrollY", {
      configurable: true,
      writable: true,
      value: -1,
    });
    window.dispatchEvent(new Event("scroll"));
    flushRaf();
  }

  function stubDistinctRanges(): void {
    loadQuranDataStub.mockResolvedValue({
      surahLocalPage: (_num: number, localPage: number) => ({
        surah: 1,
        localPage,
        globalPage: localPage,
        startGlobal: (localPage - 1) * 7 + 1,
        endGlobal: (localPage - 1) * 7 + 7,
        startAyah: 1,
        endAyah: 7,
        first: "1:1",
        last: "1:7",
      }),
      globalIndexOf: (_s: number, a: number) => a,
      surahByNum: () => SURAH,
      surahLocalPageForAyah: () => ({ localPage: 1 }),
    });
  }

  function surahOneRange(from: number, to: number) {
    const ayahs = Array.from({ length: to - from + 1 }, (_, i) => ({
      key: `1:${from + i}`,
      surah: 1,
      ayah: from + i,
      globalIndex: from + i,
      text: `v${from + i}`,
    }));
    return {
      ayahs,
      normalizations: [pageData().normalization],
    };
  }

  // W7-R2-2: a worker-degraded flag set by one adjacent read clears on a
  // subsequent clean adjacent read (the status callback re-assigns, not merges).
  it("clears workerDegraded when a later adjacent read succeeds cleanly", async () => {
    stubDistinctRanges();
    workerStub.readRange.mockImplementation(
      (
        from: number,
        _to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        if (from <= 8) onStatus?.({ servedBy: "local", workerFailure: { kind: "worker" } });
        else onStatus?.({ servedBy: "local" });
        return Promise.resolve(surahOneRange(from, _to));
      },
    );

    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await driveAdjacentRead();

    expect(workerStub.readRange).toHaveBeenCalled();
    let region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/local offline copy/i);

    fireForwardFill();
    await flushMicrotasks(20);

    region = target.querySelector('[role="status"]');
    expect(region).toBeNull();
  });

  // W7-R2-2 mirror: an api-degraded flag set by one adjacent read clears on a
  // subsequent clean adjacent read (the status callback re-assigns, not merges).
  it("clears apiDegraded when a later adjacent read succeeds cleanly", async () => {
    stubDistinctRanges();
    workerStub.readRange.mockImplementation(
      (
        from: number,
        _to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        if (from <= 8) onStatus?.({ servedBy: "local", apiFailure: { kind: "http", status: 503 } });
        else onStatus?.({ servedBy: "local" });
        return Promise.resolve(surahOneRange(from, _to));
      },
    );

    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await driveAdjacentRead();

    expect(workerStub.readRange).toHaveBeenCalled();
    let region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/network is slow/i);

    fireForwardFill();
    await flushMicrotasks(20);

    region = target.querySelector('[role="status"]');
    expect(region).toBeNull();
  });

  // W7 both-down: worker AND API failing in one status surfaces a clearable
  // degraded state (worker copy wins the template if/else; status re-assigns, not merges).
  it("surfaces worker+API both-down as a single clearable degraded state", async () => {
    stubDistinctRanges();
    workerStub.readRange.mockImplementation(
      (
        from: number,
        _to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        if (from <= 8) {
          onStatus?.({
            servedBy: "api",
            workerFailure: { kind: "worker" },
            apiFailure: { kind: "http", status: 503 },
          });
        } else {
          onStatus?.({ servedBy: "local" });
        }
        return Promise.resolve(surahOneRange(from, _to));
      },
    );

    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await driveAdjacentRead();

    expect(workerStub.readRange).toHaveBeenCalled();
    let region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/local offline copy/i);
    expect(region?.textContent ?? "").not.toMatch(/couldn't be loaded/i);

    fireForwardFill();
    await flushMicrotasks(20);

    region = target.querySelector('[role="status"]');
    expect(region).toBeNull();
  });

  // W7-R2-3: a route-key change (navigation) discards stale degraded state.
  // Svelte 5 removed imperative $set on mounted instances, so the in-place
  // route-key $effect (which clears degraded when `initial` changes on a LIVING
  // instance) cannot be driven here without a wrapper component. This covers the
  // real navigation path — SvelteKit remounts the route with a new initial — and
  // asserts the new instance starts clean and owns independent degradation state.
  it("does not carry degraded state from a prior route into a fresh mount", async () => {
    stubDistinctRanges();
    workerStub.readRange.mockImplementation(
      (
        from: number,
        to: number,
        _validate?: AyahCoordinateValidator,
        _source?: QuranReaderSource,
        onStatus?: (status: ReadTierStatus) => void,
      ) => {
        onStatus?.({ servedBy: "local", workerFailure: { kind: "worker" } });
        return Promise.resolve(surahOneRange(from, to));
      },
    );

    const first = mount(SurahReader, {
      target,
      props: propsFor(pageData({ ayahs: 7, pageCount: 3 })),
    });
    await driveAdjacentRead();
    expect(target.querySelector('[role="status"]')?.textContent ?? "").toMatch(
      /local offline copy/i,
    );
    await unmount(first);

    const next = document.createElement("div");
    document.body.appendChild(next);
    mount(SurahReader, {
      target: next,
      props: propsFor(pageData({ localPage: 2, ayahs: 7, pageCount: 3 })),
    });
    await flushMicrotasks(20);
    expect(next.querySelector('[role="status"]')).toBeNull();
    expect(target.querySelector('[role="status"]')).toBeNull();

    flushRaf();
    await flushMicrotasks(20);
    expect(next.querySelector('[role="status"]')?.textContent ?? "").toMatch(/local offline copy/i);
  });

  // W7-R2-4: regression for the round-1 loadFailed-scoping fix — a successful
  // adjacent-page load must NOT blanket-clear a different page's loadFailed.
  // Page 3 fails (failedPage=3); loading page 1 succeeds and must leave page 3's
  // inline retry visible.
  it("keeps a different page's loadFailed + retry visible after an adjacent success", async () => {
    stubDistinctRanges();
    workerStub.readRange.mockImplementation((from: number) => {
      if (from >= 15) return Promise.reject(new Error("boom")); // page 3 fails
      return Promise.resolve(surahOneRange(from, from + 6)); // page 1 succeeds
    });

    mount(SurahReader, {
      target,
      props: propsFor(pageData({ localPage: 2, ayahs: 7, pageCount: 3 })),
    });
    await driveAdjacentRead();

    let region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/Page 3 couldn't be loaded/i);
    expect(target.querySelector('button[type="button"]')).not.toBeNull();

    scrollUp();
    await flushMicrotasks(20);

    region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/Page 3 couldn't be loaded/i);
    expect(target.querySelector('button[type="button"]')).not.toBeNull();
  });
});

describe("SurahReader W7-R2-1 retry-button gate", () => {
  it("hides Retry when loadFailed has no actionable target (continueReading failure)", async () => {
    const full = pageData({ ayahs: 7, pageCount: 3 });
    readerStub.hasLastRead = true;
    readerStub.lastRead = { num: 1, n: 1, sourceId: "uthmani" };
    loadQuranDataStub.mockRejectedValue(new Error("boom"));

    mount(SurahReader, { target, props: propsFor(full) });
    await flushMicrotasks();

    const continueBtn = Array.from(target.querySelectorAll("button")).find((b) =>
      /continue reading/i.test(b.textContent ?? ""),
    );
    continueBtn?.click();
    await flushMicrotasks(20);

    const region = target.querySelector('[role="status"]');
    expect(region).not.toBeNull();
    expect(region?.querySelector('button[type="button"]')).toBeNull();
  });

  it("shows Retry for a real failed adjacent page", async () => {
    const full = pageData({ ayahs: 7, pageCount: 3 });
    workerStub.readRange.mockRejectedValue(new Error("boom"));

    mount(SurahReader, { target, props: propsFor(full) });
    await flushMicrotasks();
    flushRaf();
    await flushMicrotasks(20);

    const region = target.querySelector('[role="status"]');
    expect(region?.textContent ?? "").toMatch(/couldn't be loaded/i);
    expect(region?.querySelector('button[type="button"]')).not.toBeNull();
  });
});
