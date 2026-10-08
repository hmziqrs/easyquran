import { mount, unmount } from "svelte";
import type { ComponentProps } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { QuranReaderSource } from "#lib/data/quran-types.js";
import type { SurahLocalPageData } from "#lib/data/quran.js";
import type { ReadTierStatus } from "#lib/quran/fetch.js";
import type { AyahCoordinateValidator } from "#lib/quran/wire.js";

// Minimal surface of the ReaderHeader props the tests drive.
interface HeaderStubProps {
  onChangeMode: (mode: "verse" | "reading") => void;
  readingText: "arabic" | "translation";
  readingFlowName: string | null;
  onReadArabic: () => void;
  onReadTranslation: () => void;
  onPickTranslation: () => void;
}

// ---- hoisted doubles -------------------------------------------------------
const {
  nav,
  workerStub,
  loadQuranDataStub,
  quranStore,
  refreshAllSpy,
  gotoSpy,
  replaceStateSpy,
  readerStub,
  mountStub,
  headerProps,
  rowProps,
  setSourceIdSpy,
} = vi.hoisted(() => ({
  nav: { state: {}, url: new URL("https://example.test/al-fatihah") },
  workerStub: {
    ready: true,
    whenReady: vi.fn(),
    readRange: vi.fn(),
    onStatus: vi.fn().mockReturnValue(() => {}),
  },
  loadQuranDataStub: vi.fn(),
  // SAFETY: the double mirrors the real store contract; beforeEach reassigns status to arbitrary status strings and error to string | null, and both seed values are members of those unions.
  quranStore: { status: "idle" as string, error: null as string | null },
  refreshAllSpy: vi.fn().mockResolvedValue(undefined),
  gotoSpy: vi.fn().mockResolvedValue(undefined),
  replaceStateSpy: vi.fn(),
  readerStub: {
    hasLastRead: false,
    // Mirrors the real store contract's default mushaf script (schema v4 field).
    arabicScript: "uthmani",
    // SAFETY: tests below reassign lastRead to { num, n, sourceId } objects or null; null is a member of that union.
    lastRead: null as { num: number; n: number; sourceId?: string } | null,
    lastReadRef: "",
    // SAFETY: null is a member of the seeded union; no test in this file ever sets a concrete anchor.
    lastReadAnchor: null as { verseKey: string; localPage: number; ratio: number } | null,
    markRead: vi.fn(),
    setLastReadAnchor: vi.fn(),
    consumePendingAnchor: vi.fn(() => null),
    seedAyahs: vi.fn(),
    position: null,
    setPosition: vi.fn(),
    mode: "verse",
    isReadingMode: false,
    isVerseMode: true,
    setMode: vi.fn(),
  },
  mountStub: () => {},
  // Real ReaderHeader props captured on mount so tests can drive onChangeMode
  // exactly like the header's mode pills do.
  // SAFETY: null is the not-yet-mounted member of the nullable holder; tests assign the captured HeaderStubProps on mount.
  headerProps: { current: null as HeaderStubProps | null },
  // VerseRow props captured per mount so tests can read what each row was given.
  // SAFETY: an empty array literal of the captured-props record; tests only push stub props objects.
  rowProps: [] as {
    isTranslation?: boolean;
    arabicPending?: boolean;
    stacked?: readonly { sourceId: string }[];
  }[],
  setSourceIdSpy: vi.fn(),
}));

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/navigation", () => ({
  beforeNavigate: () => {},
  goto: gotoSpy,
  refreshAll: refreshAllSpy,
  replaceState: replaceStateSpy,
}));
// kit 3: base is gone; resolve() prefixes the (always-empty) base with a slash.
vi.mock("$app/paths", () => ({ resolve: (p: string) => `/${p}` }));
vi.mock("$app/state", () => ({ page: nav }));

vi.mock("#lib/data/quran-data-client.js", () => ({
  loadQuranData: loadQuranDataStub,
  peekQuranData: () => undefined,
}));
vi.mock("#lib/quran/worker-client.js", () => ({ quranWorker: workerStub }));
vi.mock("#lib/quran/catalogue.js", () => {
  const entry = (id: string) => ({
    id,
    language: id.startsWith("ur.") ? "Urdu" : "English",
    languageCode: id.startsWith("ur.") ? "ur" : "en",
    direction: "ltr" as const,
    name: `Name ${id}`,
    translator: `Translator ${id}`,
    sizeBytes: 2048,
    downloadUrl: "",
  });
  const list = ["en.sahih", "en.arberry", "ur.jalandhry"].map((id) => entry(id));
  return {
    TRANSLATION_CATALOGUE: list,
    TRANSLATION_CATALOGUE_BY_ID: new Map(list.map((t) => [t.id, t])),
    flagFor: () => ({ flag: "", country: "" }),
    nativeNameFor: () => null,
    translationSourceOf: () => "tanzil",
  };
});
vi.mock("#lib/stores/reader-settings.svelte.js", () => ({
  readerSource: { sourceId: null, setSourceId: setSourceIdSpy },
}));
vi.mock("#lib/quran/engagement.js", () => ({
  noteTranslationChosen: vi.fn(() => Promise.resolve()),
}));
vi.mock("#lib/stores/quran.svelte.js", () => ({ quran: quranStore }));
vi.mock("#lib/stores/reader.svelte.js", () => ({
  reader: readerStub,
  ReaderMode: { Reading: "reading", Verse: "verse" },
}));

// child components as trivial stubs so mount never depends on their internals.
vi.mock("../ReaderHeader.svelte", () => ({
  default: (...args: unknown[]) => {
    // SAFETY: Svelte 5 invokes child components as (anchor, props); the props object is always the last argument, so the assertion only widens unknown[] back to the stub contract.
    headerProps.current = args[args.length - 1] as HeaderStubProps;
  },
}));
vi.mock("../ReaderPageNav.svelte", () => ({ default: mountStub }));
vi.mock("../VerseRow.svelte", () => ({
  default: (...args: unknown[]) => {
    // SAFETY: Svelte 5 invokes child components as (anchor, props); the props object is the last argument.
    rowProps.push(args[args.length - 1] as (typeof rowProps)[number]);
  },
}));

// ---- helpers ---------------------------------------------------------------
function flushMicrotasks(n = 12): Promise<void> {
  let p = Promise.resolve();
  for (let i = 0; i < n; i++) p = p.then(() => undefined);
  return p;
}

const SURAH = { num: 1, slug: "al-fatihah", name: "Al-Fatihah", arabic: "الفاتحة" };

// SurahLocalPageData with a minimal valid shape.
function pageData(opts: { localPage?: number; ayahs?: number; pageCount?: number } = {}) {
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
  workerStub.whenReady.mockReset().mockResolvedValue(undefined);
  loadQuranDataStub.mockReset();
  loadQuranDataStub.mockResolvedValue({
    surahLocalPage: () => ({ startGlobal: 8, endGlobal: 14 }),
    globalIndexOf: (_s: number, a: number) => a,
    surahByNum: () => SURAH,
    surahLocalPageForAyah: () => ({ localPage: 1 }),
  });
  refreshAllSpy.mockReset().mockResolvedValue(undefined);
  gotoSpy.mockReset().mockResolvedValue(undefined);
  replaceStateSpy.mockClear();
  quranStore.status = "idle";
  quranStore.error = null;
  readerStub.seedAyahs.mockReset();
  readerStub.markRead.mockReset();
  readerStub.hasLastRead = false;
  readerStub.lastRead = null;
  readerStub.mode = "verse";
  readerStub.isReadingMode = false;
  readerStub.isVerseMode = true;
  readerStub.setMode = vi.fn().mockImplementation((m: "verse" | "reading") => {
    readerStub.mode = m;
    readerStub.isReadingMode = m === "reading";
    readerStub.isVerseMode = m === "verse";
  });
  headerProps.current = null;
  setSourceIdSpy.mockReset();
  nav.url = new URL("https://example.test/al-fatihah");
  localStorage.clear();

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

describe("SurahReader anchor history", () => {
  it.each([true, false])(
    "protects a requested anchor while scrolling=%s",
    async (anchorScrolling) => {
      const view = mount(SurahReader, {
        target,
        props: { ...propsFor(pageData({ ayahs: 7, pageCount: 1 })), anchorScrolling },
      });
      for (let frame = 0; frame < 5; frame += 1) {
        await flushMicrotasks(20);
        flushRaf();
      }
      await flushMicrotasks(20);
      if (anchorScrolling) expect(replaceStateSpy).not.toHaveBeenCalled();
      else expect(replaceStateSpy).toHaveBeenCalled();
      await unmount(view);
    },
  );
});

describe("SurahReader W7 single-page empty recovery", () => {
  it("calls refreshAll exactly once then falls back to W5 readRange (no goto)", async () => {
    const empty = pageData({ ayahs: 0, pageCount: 1 });
    // readRange resolves with content so the W5 fallback is observable.
    workerStub.readRange.mockResolvedValue({
      ayahs: [{ key: "1:1", surah: 1, ayah: 1, globalIndex: 1, text: "x" }],
      normalizations: [empty.normalization],
    });

    mount(SurahReader, { target, props: propsFor(empty) });
    await flushMicrotasks(20);

    expect(refreshAllSpy).toHaveBeenCalledTimes(1);
    expect(workerStub.readRange).toHaveBeenCalled();
    // Recovery never performs document navigation.
    expect(gotoSpy).not.toHaveBeenCalled();
  });

  it("does not re-enter retryInitialPage while one is already in flight", async () => {
    const empty = pageData({ ayahs: 0, pageCount: 1 });
    // Block refreshAll so initialRetryInFlight stays true across triggers.
    let resolveInvalidate!: () => void;
    refreshAllSpy.mockReturnValue(new Promise<void>((r) => (resolveInvalidate = r)));

    mount(SurahReader, { target, props: propsFor(empty) });
    await flushMicrotasks();
    await flushMicrotasks();

    // Even after several flushes, only a single refreshAll is in flight.
    expect(refreshAllSpy).toHaveBeenCalledTimes(1);
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
    expect(refreshAllSpy).not.toHaveBeenCalled();
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

  it("retries explicit ayah jump after cold worker becomes ready", async () => {
    stubDistinctRanges();
    const geometry = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockReturnValue(new DOMRect(0, 0, 600, 9000));
    workerStub.ready = false;
    workerStub.whenReady.mockImplementation(() => {
      workerStub.ready = true;
      return Promise.resolve();
    });
    workerStub.readRange.mockImplementation((from: number, to: number) => {
      if (!workerStub.ready) return Promise.reject(new Error("worker starting"));
      return Promise.resolve(surahOneRange(from, to));
    });
    const view = mount(SurahReader, {
      target,
      props: propsFor(pageData({ ayahs: 7, pageCount: 3 })),
    });
    await flushMicrotasks(20);
    const jump = view.ensureAyah(3, "1:15");
    for (let frame = 0; frame < 8; frame += 1) {
      await flushMicrotasks(20);
      flushRaf();
    }
    await jump;
    expect(workerStub.whenReady).toHaveBeenCalledOnce();
    expect(workerStub.readRange.mock.calls.filter(([from]) => from === 15)).toHaveLength(2);
    expect(target.querySelector('[role="status"]')?.textContent ?? "").not.toMatch(
      /couldn't be loaded/i,
    );
    window.dispatchEvent(new WheelEvent("wheel", { deltaY: -500 }));
    for (let frame = 0; frame < 8; frame += 1) {
      await flushMicrotasks(20);
      flushRaf();
    }
    expect(workerStub.readRange.mock.calls.some(([from]) => from === 8)).toBe(true);
    await unmount(view);
    geometry.mockRestore();
  });

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

import { readingText } from "#lib/stores/reading-text.svelte.js";
// ---- reading mode: Arabic or one translation, never a dialog -----------------
import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";

describe("SurahReader reading mode", () => {
  function translationPageData(): ReturnType<typeof pageData> {
    const base = pageData({ ayahs: 7, pageCount: 3 });
    // Route translation instead of the default uthmani source id.
    return { ...base, normalization: { ...base.normalization, sourceId: "en.sahih" } };
  }
  function settle(): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, 40));
  }
  function header(): HeaderStubProps {
    const props = headerProps.current;
    if (!props) throw new Error("ReaderHeader never mounted");
    return props;
  }

  beforeEach(() => {
    stackedTranslations.clear();
    readingText.readArabic();
    // Stacked translations fetch through the worker; give the stub a resolving range
    // so the controller's sync effect never awaits undefined.
    workerStub.readRange.mockResolvedValue({ ayahs: [], normalizations: [] });
  });

  it("switches to reading at once with translations stacked — no dialog, no navigation", async () => {
    stackedTranslations.setIds(["en.sahih", "en.arberry"]);
    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await flushMicrotasks();
    header().onChangeMode("reading");
    await settle();
    expect(readerStub.mode).toBe("reading");
    expect(document.querySelectorAll('input[type="radio"]')).toHaveLength(0);
    expect(gotoSpy).not.toHaveBeenCalled();
  });

  it("defaults Reading to the Arabic on an Arabic page", async () => {
    stackedTranslations.setIds(["en.arberry", "ur.jalandhry"]);
    readerStub.setMode("reading");
    rowProps.length = 0;
    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await flushMicrotasks();
    expect(header().readingText).toBe("arabic");
    expect(header().readingFlowName).toBeNull();
    expect(rowProps).toHaveLength(0);
    expect(target.querySelector("[data-page-rendered]")).not.toBeNull();
  });

  it("fetches only selected Reading translation when other translations are stacked", async () => {
    stackedTranslations.setIds(["en.arberry", "ur.jalandhry"]);
    readingText.readTranslation("en.arberry");
    readerStub.setMode("reading");
    const view = mount(SurahReader, {
      target,
      props: propsFor(pageData({ ayahs: 7, pageCount: 1 })),
    });
    await flushMicrotasks(30);
    expect(workerStub.readRange).toHaveBeenCalled();
    expect(workerStub.readRange.mock.calls.every((call) => call[3] === "en.arberry")).toBe(true);
    await unmount(view);
  });

  it("flows the route's own translation by default on a translation page", async () => {
    readerStub.setMode("reading");
    mount(SurahReader, { target, props: propsFor(translationPageData()) });
    await flushMicrotasks();
    expect(header().readingText).toBe("translation");
    expect(header().readingFlowName).toBe("Name en.sahih");
  });

  it("the Translation pill flows the first stacked translation in place when nothing was picked", async () => {
    stackedTranslations.setIds(["en.arberry", "ur.jalandhry"]);
    readerStub.setMode("reading");
    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await flushMicrotasks();
    header().onReadTranslation();
    await settle();
    expect(readingText.text).toBe("translation");
    expect(header().readingFlowName).toBe("Name en.arberry");
    expect(gotoSpy).not.toHaveBeenCalled();
  });

  it("the Translation pill keeps the saved pick, even one that is not stacked", async () => {
    readingText.readTranslation("ur.jalandhry");
    readingText.readArabic();
    stackedTranslations.setIds(["en.arberry"]);
    readerStub.setMode("reading");
    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await flushMicrotasks();
    header().onReadTranslation();
    await settle();
    expect(header().readingFlowName).toBe("Name ur.jalandhry");
  });

  it("opens the full picker, picks a translation, and remembers it as recent", async () => {
    readerStub.setMode("reading");
    mount(SurahReader, { target, props: propsFor(pageData({ ayahs: 7, pageCount: 3 })) });
    await flushMicrotasks();
    header().onPickTranslation();
    await settle();
    const row = document.querySelector<HTMLButtonElement>('[data-row-pick="en.arberry"]');
    expect(row).not.toBeNull();
    row?.click();
    await settle();
    expect(readingText.text).toBe("translation");
    expect(readingText.translationId).toBe("en.arberry");
    expect(readingText.recent[0]).toBe("en.arberry");
    expect(header().readingFlowName).toBe("Name en.arberry");
  });

  it("Ayah-by-Ayah on a translation page leads with the Arabic and puts the page's translation first", async () => {
    stackedTranslations.setIds(["en.arberry"]);
    rowProps.length = 0;
    mount(SurahReader, { target, props: propsFor(translationPageData()) });
    await flushMicrotasks();
    const row = rowProps[0];
    expect(row?.isTranslation).toBe(false);
    expect(row?.arabicPending).toBe(true);
    expect(row?.stacked?.[0]?.sourceId).toBe("en.sahih");
  });

  it("Reading → Arabic on a translation page opens the Arabic surah root in reading mode", async () => {
    readerStub.setMode("reading");
    nav.url = new URL("https://example.test/al-fatihah/t/en/sahih");
    mount(SurahReader, { target, props: propsFor(translationPageData()) });
    await flushMicrotasks();
    header().onReadArabic();
    await settle();
    expect(readingText.text).toBe("arabic");
    expect(gotoSpy).toHaveBeenCalledTimes(1);
    const href = String(gotoSpy.mock.calls[0]?.[0]);
    // One URL per surah: the Arabic twin is the bare root, mode rides in query.
    expect(href).toContain("/al-fatihah");
    expect(href).not.toContain("/page/");
    expect(href).not.toContain("/t/");
    expect(href).toContain("mode=reading");
  });
});
