<script lang="ts">
  import { onMount, onDestroy, tick, untrack } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { SvelteSet } from "svelte/reactivity";
  import { beforeNavigate, invalidateAll, replaceState } from "$app/navigation";
  import { page as appPage } from "$app/state";
  import {
    parseKey,
    surahLocalPagePathFor,
    surahRouteContext,
    type SurahLocalPageData,
    type SurahLocalPageLink,
    type SurahLink,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { resumeToLastRead } from "$lib/reader/resume";
  import { Icon } from "$lib/components/icon";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import { quranWorker } from "$lib/quran/worker-client";
  import { TRANSLATION_CATALOGUE } from "$lib/quran/catalogue";
  import type { ReadTierStatus } from "$lib/quran/fetch";
  import {
    SURAH_PAGE_WINDOW_SIZE,
    virtualPageWindow,
    windowSizeForViewport,
  } from "$lib/quran/virtual-pages";
  import { bodyText } from "$lib/quran/view/source-view";
  import { headerText } from "$lib/quran/view/presentation";
  import { quran } from "$lib/stores/quran.svelte";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import { withModeParam } from "$lib/reader/mode-param";
  import { PREPARE_RELOAD, PREPARE_RELOAD_EVENT, UPDATE_BROADCAST_CHANNEL } from "$lib/offline/messages";
  import { PageHeightCache, stablePageHeight, widthBucket } from "./page-heights";
  import { ayahIndexValidator } from "./range-validate";
  import {
    parseHistoryState,
    persistReaderPosition,
    reloadPositionState,
    type SurahReaderHistoryState,
  } from "./reader-history";
  import {
    captureViewportAnchor,
    closestPage,
    nextFrame,
    restoreViewportAnchor,
    viewportMarker,
    type ViewportAnchor,
  } from "./viewport-anchor";
  import ReaderHeader from "./ReaderHeader.svelte";
  import ReaderPageNav from "./ReaderPageNav.svelte";
  import ReaderStatusBanner from "./ReaderStatusBanner.svelte";
  import { ReaderDegradationState } from "./reader-degradation.svelte";
  import VerseRow from "./VerseRow.svelte";
  import {
    createStackedTranslations,
    erroredFor,
    loadingFor,
    stackedFor,
  } from "./stacked-translations.svelte";

  let {
    initial,
    previousPage,
    nextPage,
    previousSurah,
    nextSurah,
    anchorScrolling = false,
    onVisiblePage,
  }: {
    initial: SurahLocalPageData;
    previousPage: SurahLocalPageLink | null;
    nextPage: SurahLocalPageLink | null;
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
    anchorScrolling?: boolean;
    onVisiblePage?: (pageData: SurahLocalPageData) => void;
  } = $props();

  const copy = getReaderUiCopy();

  let loadedPages = $state.raw<SurahLocalPageData[]>([]);
  const pages = $derived.by(() => {
    // `initial` can be gone for one turn when a keyed swap / hot update tears the
    // route down while a lazy read (history snapshot) still re-evaluates us.
    if (!initial) return [];
    const byPage = new Map<number, SurahLocalPageData>();
    for (const pageData of [initial, ...loadedPages]) {
      const existing = byPage.get(pageData.page.localPage);
      if (!existing || pageData.ayahs.length > 0) byPage.set(pageData.page.localPage, pageData);
    }
    return [...byPage.values()].sort((a, b) => a.page.localPage - b.page.localPage);
  });
  let readerPages: HTMLElement | null = $state(null);
  const loadingPages = new SvelteSet<number>();
  const degradation = new ReaderDegradationState();
  let initialRetryInFlight = false;
  let clientMounted = $state(false);
  let activeLocalPage = $state<number | null>(null);
  let virtualCenterPage = $state<number | null>(null);
  let readerWidth = $state(0);
  let viewportHeight = $state(0);
  let lastScrollY = 0;
  let touchY: number | null = null;
  let scrollFrame = 0;
  let forwardFillFrame = 0;
  let historyWriteTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressScroll = false;
  let sawUserInput = false;
  let userScrolled = false;
  let layoutRepairPending = false;
  let virtualShiftPage: number | null = null;
  let stableAnchor: ViewportAnchor | null = null;
  let positionQueue = Promise.resolve();
  const heightCache = new PageHeightCache();
  const loadAheadPx = 900;
  const visibleLocalPage = $derived(activeLocalPage ?? initial?.page.localPage ?? 1);
  const virtualFocusPage = $derived(virtualCenterPage ?? visibleLocalPage);
  // Rendered-page budget: small Arabic sizes make pages shorter than the viewport,
  // so scale the window to keep ~1.5 viewports rendered on each side of the focus.
  // The size must NOT feed the rendered set directly: it tracks the height cache,
  // which is written by ResizeObserver AFTER a resize — a reactive window change
  // would mount/unmount pages around the reader with no anchor restore, and the
  // document visibly jumps. The effect below applies it through the same
  // anchor-preserving queue as every other layout change.
  const adaptiveWindowSize = $derived.by(() => {
    const focusHeight = heightCache.get(virtualFocusPage, readerWidth);
    return windowSizeForViewport(viewportHeight, focusHeight);
  });
  let renderedWindowSize = $state(SURAH_PAGE_WINDOW_SIZE);

  $effect(() => {
    const next = adaptiveWindowSize;
    if (!clientMounted || next === renderedWindowSize) return;
    void preserveViewport(
      () => {
        renderedWindowSize = next;
      },
      true,
    );
  });
  const firstLoaded = $derived(pages[0]!);
  const lastLoaded = $derived(pages.at(-1)!);
  const sourceId = $derived(initial.normalization.sourceId);
  const routeContext = $derived(surahRouteContext(sourceId));
  const isTranslationSource = $derived(routeContext.kind !== "arabic");
  const routeKey = $derived(`${sourceId}:${initial.surah.num}:${initial.page.localPage}`);
  let lastRouteKey: string | null = null;
  let stackedQuranData = $state<Awaited<ReturnType<typeof loadQuranData>> | null>(null);
  const stackedController = createStackedTranslations({
    from: () => (pages.length ? Math.min(...pages.map((p) => p.page.startGlobal)) : 0),
    to: () => (pages.length ? Math.max(...pages.map((p) => p.page.endGlobal)) : 0),
    validator: () => (stackedQuranData ? ayahIndexValidator(stackedQuranData) : null),
    primarySourceId: () => (isTranslationSource ? sourceId : null),
    catalogue: () => TRANSLATION_CATALOGUE,
    routeKey: () => `${sourceId}:${initial.surah.num}`,
  });
  const stackedAnnouncement = $derived.by(() => {
    const st = stackedController.state;
    if (st.order.some((id) => st.status.get(id) === "error")) return copy.stacked.error;
    if (st.order.some((id) => st.status.get(id) === "loading")) return copy.stacked.loading;
    return "";
  });
  $effect(() => stackedController.sync());
  onDestroy(() => stackedController.dispose());
  onMount(() => {
    void loadQuranData()
      .then((qd) => {
        stackedQuranData = qd;
      })
      .catch(() => {});
  });
  function pagePathFor(localPage: number): `/${string}` {
    return readerHrefFor(
      copy.locale,
      surahLocalPagePathFor(routeContext, initial.surah, localPage),
    );
  }
  const renderedPageNumbers = $derived.by(
    () =>
      new Set(
        virtualPageWindow(
          pages.map((pageData) => pageData.page.localPage),
          virtualFocusPage,
          renderedWindowSize,
        ),
      ),
  );

  function captureAnchor(): ViewportAnchor | null {
    return captureViewportAnchor(readerPages);
  }

  function restoreAnchor(anchor: ViewportAnchor): boolean {
    if (restoreViewportAnchor(readerPages, anchor)) {
      lastScrollY = window.scrollY;
      return true;
    }
    return false;
  }

  function markAnchorRead(anchor: ViewportAnchor | null | undefined): void {
    if (!userScrolled || anchor?.kind !== "verse") return;
    const { num, n } = parseKey(anchor.verseKey);
    if (num === initial.surah.num) {
      reader.markRead(num, n, sourceId);
      reader.setLastReadAnchor({ verseKey: anchor.verseKey, localPage: anchor.localPage, ratio: anchor.ratio });
    }
  }

  function measurePage(localPage: number): Attachment<HTMLElement> {
    return (node) => {
      let lastTotalHeight = 0;
      const measure = () => {
        const rect = node.getBoundingClientRect();
        const parentWidth = readerPages?.getBoundingClientRect().width ?? rect.width;
        const totalHeight = rect.height;
        heightCache.save(localPage, stablePageHeight(node, rect), parentWidth);
        if (
          lastTotalHeight > 0 &&
          Math.abs(totalHeight - lastTotalHeight) > 1 &&
          clientMounted &&
          !suppressScroll &&
          !layoutRepairPending &&
          stableAnchor &&
          rect.bottom > 0 &&
          rect.top < window.innerHeight
        ) {
          const anchor = stableAnchor;
          layoutRepairPending = true;
          void preserveViewportFrom(() => anchor, () => undefined, true).finally(() => {
            layoutRepairPending = false;
          });
        }
        lastTotalHeight = totalHeight;
      };
      const observer = new ResizeObserver(measure);
      observer.observe(node);
      measure();
      return () => observer.disconnect();
    };
  }

  function preserveViewportFrom(
    anchorSource: () => ViewportAnchor | null,
    change: () => void,
    waitForLayout = false,
  ): Promise<void> {
    const operation = async () => {
      const startScrollY = window.scrollY;
      // Document top is a hard invariant: at scrollY 0 nothing above the
      // viewport exists that could reflow, so any scroll away from 0 after a
      // change is pure jump. Skip the anchor entirely (its nearest-node
      // fallback is what dragged the first ayah up to the marker) and pin the
      // offset back to 0 in the finally block, after the browser has clamped.
      const atTop = startScrollY <= 0;
      const anchor = atTop ? null : anchorSource();
      suppressScroll = true;
      try {
        change();
        await tick();
        if (waitForLayout) await nextFrame();
        if (anchor) {
          restoreAnchor(anchor);
          // Layout settles a frame late when text metrics change (font
          // resize, note toggle, page swaps). One restore is a guess; keep
          // re-applying until the tracked point is stable across a frame —
          // the restore itself converges because it is relative.
          if (waitForLayout) {
            for (let settle = 0; settle < 3; settle += 1) {
              const before = window.scrollY;
              await nextFrame();
              if (!restoreAnchor(anchor)) break;
              if (Math.abs(window.scrollY - before) <= 0.5) break;
            }
          }
        }
        await nextFrame();
        updateVisiblePage();
        stableAnchor = captureAnchor();
      } finally {
        suppressScroll = false;
        if (atTop) window.scrollTo(0, 0);
        // onScroll early-returns while suppressed, so lastScrollY is stale by
        // however much the restore moved; resync it or the next real scroll
        // computes a phantom direction and warms the wrong side.
        lastScrollY = window.scrollY;
      }
    };
    const result = positionQueue.then(operation, operation);
    positionQueue = result.catch(() => undefined);
    return result;
  }

  function preserveViewport(change: () => void, waitForLayout = false): Promise<void> {
    return preserveViewportFrom(captureAnchor, change, waitForLayout);
  }

  const captureReaderPages: Attachment<HTMLElement> = (node) => {
    readerPages = node;
    const updateWidth = () => {
      const nextWidth = Math.round(node.getBoundingClientRect().width);
      if (nextWidth > 0 && widthBucket(nextWidth) !== widthBucket(readerWidth)) {
        const anchor = stableAnchor;
        readerWidth = nextWidth;
        if (
          clientMounted &&
          anchor &&
          node.getBoundingClientRect().bottom > 0 &&
          node.getBoundingClientRect().top < window.innerHeight
        ) {
          void preserveViewportFrom(
            () => anchor,
            () => {
              virtualCenterPage = visibleLocalPage;
            },
            true,
          );
        }
      } else if (readerWidth === 0) {
        readerWidth = nextWidth;
      }
      scheduleForwardFill();
    };
    const observer = new ResizeObserver(updateWidth);
    observer.observe(node);
    updateWidth();
    return () => {
      observer.disconnect();
      if (readerPages === node) readerPages = null;
    };
  };

  function shiftVirtualWindow(localPage: number): void {
    if (renderedPageNumbers.has(localPage) || virtualShiftPage === localPage) return;
    virtualShiftPage = localPage;
    // SAFETY: document.activeElement is Element | null; only page/render elements can hold focus in
    // the reader, and closest<HTMLElement> types the match for dataset.localPage access below.
    const focusedPage = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>(
      "[data-page-rendered]",
    );
    if (
      focusedPage &&
      Math.abs(Number(focusedPage.dataset.localPage) - localPage) > 2 &&
      readerPages
    ) {
      readerPages.focus({ preventScroll: true });
    }
    void preserveViewport(() => {
      virtualCenterPage = localPage;
    }).finally(() => {
      virtualShiftPage = null;
    });
  }

  function warmVirtualWindow(direction: number): void {
    if (!readerPages || direction === 0) return;
    const candidates = [
      ...readerPages.querySelectorAll<HTMLElement>("[data-page-spacer]"),
    ].filter((spacer) => {
      const pageNumber = Number(spacer.dataset.localPage);
      const rect = spacer.getBoundingClientRect();
      return (
        rect.bottom > -loadAheadPx &&
        rect.top < window.innerHeight + loadAheadPx &&
        (direction > 0 ? pageNumber > virtualFocusPage : pageNumber < virtualFocusPage)
      );
    });
    candidates.sort((a, b) => {
      const aPage = Number(a.dataset.localPage);
      const bPage = Number(b.dataset.localPage);
      return direction > 0 ? aPage - bPage : bPage - aPage;
    });
    const localPage = Number(candidates[0]?.dataset.localPage);
    if (Number.isSafeInteger(localPage)) shiftVirtualWindow(localPage);
  }

  function changeMode(mode: ReaderMode): void {
    if (reader.mode === mode) return;
    void preserveViewport(() => {
      virtualCenterPage = visibleLocalPage;
      reader.setMode(mode);
      replaceState(withModeParam(appPage.url, mode), appPage.state);
    }, true);
  }

  function changeTypography(change: () => void): void {
    // Deliberately NO virtualCenterPage recenter here: the rendered window is
    // already centred on the reader's position, and recomputing it from the
    // (possibly stale) activeLocalPage can drop the anchor's page from the
    // rendered set mid-preserve — the document then shifts under the restored
    // scroll and the reader ends up at the top. A font resize only reflows
    // text in place; the window does not need to move.
    void preserveViewport(change, true);
  }

  function toggleNote(verseKey: string): void {
    void preserveViewport(() => {
      reader.toggleNote(verseKey);
    }, true);
  }

  function cachePage(pageData: SurahLocalPageData): void {
    reader.seedAyahs(
      pageData.ayahs.map((ayah) => ({
        key: ayah.key,
        text: bodyText(ayah.text, ayah.ayah, pageData.normalization),
      })),
    );
  }

  function historySnapshot(localPage = visibleLocalPage): SurahReaderHistoryState {
    const pageNumbers = virtualPageWindow(
      pages.map((pageData) => pageData.page.localPage),
      localPage,
      renderedWindowSize,
    );
    const included = new Set(pageNumbers);
    return {
      version: 1,
      surahNum: initial.surah.num,
      activeLocalPage: localPage,
      pages: pages.filter((pageData) => included.has(pageData.page.localPage)),
      anchor: captureAnchor(),
    };
  }

  let lastWrittenUrl: string | null = null;
  let lastWrittenPage: number | null = null;

  function writeHistoryState(
    url: string | URL = window.location.href,
    localPage = visibleLocalPage,
  ): void {
    // Guard the whole write: after a keyed swap / hot update the prop can already
    // be gone while beforeNavigate or a settled loadPage still calls in. A skipped
    // history write is harmless; a snapshot of a half-torn reader is a crash.
    if (!initial) return;
    const snapshot = historySnapshot(localPage);
    const next = withModeParam(url, reader.mode, window.location.href);
    const target = next.href;
    if (target !== lastWrittenUrl || localPage !== lastWrittenPage) {
      replaceState(next, {
        ...appPage.state,
        surahReader: snapshot,
      });
      lastWrittenUrl = target;
      lastWrittenPage = localPage;
    }
    persistReaderPosition(snapshot);
    markAnchorRead(snapshot.anchor);
  }

  function scheduleHistoryWrite(): void {
    if (historyWriteTimer) clearTimeout(historyWriteTimer);
    historyWriteTimer = setTimeout(() => {
      historyWriteTimer = null;
      writeHistoryState();
    }, 180);
  }

  onDestroy(() => {
    // The timer must never fire into a torn-down reader (same guard class as
    // writeHistoryState, but cheaper to stop at the source).
    if (historyWriteTimer) {
      clearTimeout(historyWriteTimer);
      historyWriteTimer = null;
    }
  });

  async function restoreHistory(): Promise<void> {
    const saved =
      parseHistoryState(appPage.state.surahReader, initial.surah.num) ??
      reloadPositionState(initial);
    if (saved) {
      suppressScroll = true;
      try {
        await restoreHistoryFrom(saved);
        await nextFrame();
      } finally {
        suppressScroll = false;
      }
      return;
    }
    const pending = reader.consumePendingAnchor();
    if (!pending || parseKey(pending.verseKey).num !== initial.surah.num) return;
    const anchor: ViewportAnchor = {
      kind: "verse",
      localPage: pending.localPage,
      verseKey: pending.verseKey,
      viewportPoint: viewportMarker(),
      ratio: pending.ratio,
    };
    suppressScroll = true;
    try {
      await nextFrame();
      await nextFrame();
      restoreAnchor(anchor);
      await document.fonts.ready;
      await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 80));
      restoreAnchor(anchor);
      stableAnchor = captureAnchor();
    } finally {
      suppressScroll = false;
    }
  }

  async function restoreHistoryFrom(saved: SurahReaderHistoryState): Promise<void> {
    const byPage = new Map<number, SurahLocalPageData>();
    for (const pageData of saved.pages) {
      if (
        pageData.surah?.num === initial.surah.num &&
        Number.isSafeInteger(pageData.page?.localPage) &&
        pageData.page.localPage >= 1 &&
        pageData.page.localPage <= initial.pageCount
      ) {
        byPage.set(pageData.page.localPage, pageData);
      }
    }
    byPage.delete(initial.page.localPage);
    loadedPages = [...byPage.values()];
    activeLocalPage = saved.activeLocalPage;
    virtualCenterPage = saved.activeLocalPage;
    for (const pageData of saved.pages) cachePage(pageData);
    await tick();
    onVisiblePage?.(
      saved.pages.find((pageData) => pageData.page.localPage === saved.activeLocalPage) ?? initial,
    );
    await nextFrame();
    await nextFrame();
    if (saved.anchor) restoreAnchor(saved.anchor);
    await document.fonts.ready;
    await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 80));
    if (saved.anchor) restoreAnchor(saved.anchor);
    stableAnchor = captureAnchor();
    updateVisiblePage();
  }

  async function loadPage(localPage: number): Promise<void> {
    if (
      localPage < 1 ||
      localPage > initial.pageCount ||
      pages.some((item) => item.page.localPage === localPage && item.ayahs.length > 0) ||
      loadingPages.has(localPage)
    ) {
      return;
    }
    loadingPages.add(localPage);
    // Do NOT blanket-clear degradation.loadFailed here: an adjacent-page load must
    // not hide an already-failed page's inline retry. Failure clears only on this
    // page's own success (below) or on route change.
    const readRouteKey = routeKey;
    try {
      const quranData = await loadQuranData();
      const pageDataRange = quranData.surahLocalPage(initial.surah.num, localPage);
      if (!pageDataRange) {
        throw new Error(`Unknown Surah page ${initial.surah.num}:${localPage}`);
      }
      const range = await quranWorker.readRange(
        pageDataRange.startGlobal,
        pageDataRange.endGlobal,
        ayahIndexValidator(quranData),
        isTranslationSource ? sourceId : undefined,
        (status: ReadTierStatus) => {
          if (readRouteKey !== routeKey) return;
          degradation.applyTierStatus(status);
        },
      );
      if (readRouteKey !== routeKey) return;
      const normalization = range.normalizations.find(
        (value) => value.surah === initial.surah.num,
      );
      if (!normalization || range.ayahs.some((ayah) => ayah.surah !== initial.surah.num)) {
        throw new Error(`Invalid Surah page ${initial.surah.num}:${localPage}`);
      }
      const pageData: SurahLocalPageData = {
        surah: initial.surah,
        page: pageDataRange,
        pageCount: initial.pageCount,
        ayahs: range.ayahs,
        normalization,
      };
      cachePage(pageData);
      await preserveViewport(() => {
        loadedPages = [...loadedPages, pageData];
      });
      degradation.clearIfMatches(localPage);
      if (clientMounted) writeHistoryState();
    } catch {
      if (readRouteKey === routeKey) {
        degradation.markPageFailed(localPage);
      }
    } finally {
      loadingPages.delete(localPage);
    }
  }

  async function retryInitialPage(): Promise<void> {
    if (initialRetryInFlight) return;
    initialRetryInFlight = true;
    const startKey = routeKey;
    try {
      await invalidateAll();
      if (startKey !== routeKey) return;
      if (initial.ayahs.length === 0) void loadPage(initial.page.localPage);
    } catch {
      if (startKey === routeKey && initial.ayahs.length === 0) {
        void loadPage(initial.page.localPage);
      }
    } finally {
      initialRetryInFlight = false;
    }
  }

  function retryDegradedPage(): void {
    if (initial.ayahs.length === 0) {
      void retryInitialPage();
    } else if (degradation.failedPage !== null) {
      degradation.loadFailed = false;
      void loadPage(degradation.failedPage);
    }
  }

  function requestPreviousPage(): void {
    const localPage = firstLoaded.page.localPage - 1;
    if (localPage >= 1) void loadPage(localPage);
  }

  function requestNextPage(): void {
    const localPage = lastLoaded.page.localPage + 1;
    if (localPage <= initial.pageCount) void loadPage(localPage);
  }

  function scheduleForwardFill(): void {
    if (forwardFillFrame) return;
    forwardFillFrame = requestAnimationFrame(() => {
      forwardFillFrame = 0;
      if (!readerPages) return;
      if (readerPages.getBoundingClientRect().bottom - window.innerHeight < loadAheadPx) {
        requestNextPage();
      }
    });
  }

  function setVisiblePage(localPage: number, anchor?: ViewportAnchor | null): void {
    if (!renderedPageNumbers.has(localPage)) shiftVirtualWindow(localPage);
    if (localPage === visibleLocalPage) return;
    activeLocalPage = localPage;
    const pageData = pages.find((item) => item.page.localPage === localPage);
    if (pageData) onVisiblePage?.(pageData);
    markAnchorRead(anchor !== undefined ? anchor : captureAnchor());
    writeHistoryState(publicHref(pagePathFor(localPage)), localPage);
  }

  function updateVisiblePage(anchor: ViewportAnchor | null = null): void {
    if (anchor) {
      setVisiblePage(anchor.localPage, anchor);
      return;
    }
    if (!readerPages) return;
    const section = closestPage(readerPages, viewportMarker());
    const localPage = Number(section?.dataset.localPage);
    if (Number.isSafeInteger(localPage)) setVisiblePage(localPage);
  }

  function processScroll(direction: number): void {
    scrollFrame = 0;
    if (suppressScroll || anchorScrolling) {
      stableAnchor = captureAnchor();
    } else {
      if (sawUserInput) userScrolled = true;
      warmVirtualWindow(direction);
      const anchor = captureAnchor();
      updateVisiblePage(anchor);
      stableAnchor = anchor;
      scheduleHistoryWrite();
    }
    if (!readerPages) return;
    const rect = readerPages.getBoundingClientRect();
    if (direction < 0 && rect.top > -loadAheadPx) requestPreviousPage();
    if (direction > 0 && rect.bottom - window.innerHeight < loadAheadPx) requestNextPage();
  }

  function onScroll(): void {
    const currentY = window.scrollY;
    if (suppressScroll || anchorScrolling) {
      lastScrollY = currentY;
      stableAnchor = captureAnchor();
      scheduleForwardFill();
      return;
    }
    const direction = Math.sign(currentY - lastScrollY);
    lastScrollY = currentY;
    if (direction !== 0 && sawUserInput) userScrolled = true;
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
    scrollFrame = requestAnimationFrame(() => processScroll(direction));
  }

  function atScrollBoundary(direction: number): boolean {
    if (direction < 0) return window.scrollY <= 1;
    return window.scrollY >= document.documentElement.scrollHeight - window.innerHeight - 1;
  }

  function onUserInput(): void {
    sawUserInput = true;
  }

  function onWheel(event: WheelEvent): void {
    sawUserInput = true;
    const direction = Math.sign(event.deltaY);
    if (direction !== 0 && atScrollBoundary(direction)) processScroll(direction);
  }

  function onTouchStart(event: TouchEvent): void {
    sawUserInput = true;
    touchY = event.touches[0]?.clientY ?? null;
  }

  function onTouchMove(event: TouchEvent): void {
    const nextY = event.touches[0]?.clientY;
    if (touchY === null || nextY === undefined) return;
    const direction = Math.sign(touchY - nextY);
    touchY = nextY;
    if (direction !== 0 && atScrollBoundary(direction)) processScroll(direction);
  }

  function onTouchEnd(): void {
    touchY = null;
  }

  function onResize(): void {
    viewportHeight = window.innerHeight;
    scheduleForwardFill();
  }

  function onKeyDown(event: KeyboardEvent): void {
    // SAFETY: window-level keydown; target is the focused element (or null), and only the
    // HTMLElement fields isContentEditable/tagName are read to skip text-input contexts.
    const target = event.target as HTMLElement | null;
    if (
      target?.isContentEditable ||
      target?.tagName === "INPUT" ||
      target?.tagName === "TEXTAREA" ||
      target?.tagName === "SELECT"
    ) {
      return;
    }
    sawUserInput = true;
    if (event.key === "ArrowUp" || event.key === "PageUp" || event.key === "Home") {
      processScroll(-1);
    } else if (event.key === "ArrowDown" || event.key === "PageDown" || event.key === "End") {
      processScroll(1);
    }
  }

  async function continueReading(): Promise<void> {
    if (!reader.hasLastRead) return;
    const ok = await resumeToLastRead(routeContext);
    if (!ok && reader.hasLastRead) degradation.loadFailed = true;
  }

  beforeNavigate(() => {
    if (!clientMounted) return;
    if (historyWriteTimer) {
      clearTimeout(historyWriteTimer);
      historyWriteTimer = null;
    }
    writeHistoryState();
  });

  $effect(() => {
    const key = routeKey;
    if (lastRouteKey !== null && lastRouteKey !== key) {
      degradation.reset();
    }
    lastRouteKey = key;
  });

  let lastTypography: string | null = null;
  $effect(() => {
    const typography = `${reader.arabicFont}:${reader.arabicSizePx}:${reader.translationSizePx}:${reader.translationFamily}`;
    if (lastTypography === typography) return;
    const firstRun = lastTypography === null;
    lastTypography = typography;
    if (firstRun) return;
    void preserveViewport(() => {
      virtualCenterPage = visibleLocalPage;
    }, true);
  });

  onMount(() => {
    clientMounted = true;
    viewportHeight = window.innerHeight;
    lastScrollY = window.scrollY;
    cachePage(initial);
    if (initial.ayahs.length === 0) void retryInitialPage();
    void restoreHistory().then(() => {
      stableAnchor = captureAnchor();
      scheduleForwardFill();
      void nextFrame().then(() => writeHistoryState());
    });
    const stop = quranWorker.onStatus((status) => {
      if (status !== "ready") return;
      scheduleForwardFill();
    });
    let updateChannel: BroadcastChannel | null = null;
    if ("BroadcastChannel" in window) {
      updateChannel = new BroadcastChannel(UPDATE_BROADCAST_CHANNEL);
      updateChannel.addEventListener("message", (event) => {
        if (event.data?.type === PREPARE_RELOAD) writeHistoryState();
      });
    }
    const onPrepareReload = (): void => writeHistoryState();
    window.addEventListener(PREPARE_RELOAD_EVENT, onPrepareReload);
    scheduleForwardFill();
    return () => {
      stop();
      updateChannel?.close();
      window.removeEventListener(PREPARE_RELOAD_EVENT, onPrepareReload);
      if (scrollFrame) cancelAnimationFrame(scrollFrame);
      if (forwardFillFrame) cancelAnimationFrame(forwardFillFrame);
      if (historyWriteTimer) clearTimeout(historyWriteTimer);
    };
  });
</script>

<svelte:window
  onscroll={onScroll}
  onwheel={onWheel}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  onkeydown={onKeyDown}
  onpointerdown={onUserInput}
  onresize={onResize}
/>

<div class="reader-stack flex flex-col gap-4">
  {#if clientMounted && initial.ayahs.length === 0 && !degradation.loadFailed && quran.status !== "error"}
    <span class="sr-only" role="status" aria-live="polite">{copy.shell.opening}</span>
  {/if}

  {#if reader.hasLastRead}
    <button
      type="button"
      onclick={continueReading}
      aria-label={copy.shell.continueReading(reader.lastReadRef)}
      class="flex items-center gap-3 rounded-md bg-primary-soft px-[18px] py-[13px] text-start transition-[filter] duration-150 hover:brightness-[0.98]"
    >
      <Icon name="play" size={15} class="flex-none text-primary" />
      <span class="text-sm text-primary">{copy.shell.continueReading(reader.lastReadRef)}</span>
      <span class="ms-auto text-[13px] text-primary">{copy.shell.jump} <span aria-hidden="true">→</span></span>
    </button>
  {/if}

  <div class="overflow-hidden rounded-lg border border-border bg-reader-background">
    <ReaderHeader
      {initial}
      {visibleLocalPage}
      {clientMounted}
      onChangeMode={changeMode}
      onSmaller={() => changeTypography(() => reader.smaller())}
      onBigger={() => changeTypography(() => reader.bigger())}
    />

    <div class="sr-only" aria-live="polite">{stackedAnnouncement}</div>
    <div
      {@attach captureReaderPages}
      class="reader-pages"
      data-source-kind={isTranslationSource ? "translation" : "arabic"}
      tabindex="-1"
    >
      <TooltipProvider delayDuration={300}>
        {#each pages as pageData (pageData.page.localPage)}
          {#if renderedPageNumbers.has(pageData.page.localPage)}
            <section
              class="surah-page"
              data-local-page={pageData.page.localPage}
              data-page-rendered
              aria-labelledby="surah-page-{pageData.page.localPage}-title"
              {@attach measurePage(pageData.page.localPage)}
            >
              <h2 id="surah-page-{pageData.page.localPage}-title" class="sr-only">
                {copy.shell.surahPageTitle(initial.surah.name, pageData.page.localPage, initial.pageCount)}
              </h2>
              {#if pageData.page.startAyah === 1 && headerText(pageData.normalization)}
                <p dir="rtl" lang="ar" class="surah-opener text-center text-quran-foreground">
                  {headerText(pageData.normalization)}
                </p>
              {/if}
              <ol class="ayah-list list-none p-0">
                {#each pageData.ayahs as ayah (ayah.key)}
                  <VerseRow
                    text={bodyText(ayah.text, ayah.ayah, pageData.normalization)}
                    n={ayah.ayah}
                    vKey={ayah.key}
                    onToggleNote={() => toggleNote(ayah.key)}
                    stacked={stackedFor(stackedController.state, ayah.key)}
                    stackedPending={loadingFor(stackedController.state, ayah.key)}
                    stackedErrored={erroredFor(stackedController.state, ayah.key)}
                    stackedErrorLabel={copy.stacked.error}
                  />
                {/each}
              </ol>
            </section>
          {:else}
            <!-- Untracked on purpose: a reactive spacer re-reads the height cache
                 while the ResizeObserver writes post-resize measurements, and each
                 silent correction shifts the space above the reader — the jump.
                 Spacers snapshot at render time; every swap to a real page runs
                 through the anchor-preserving queue. -->
            <div
              class="page-spacer"
              data-local-page={pageData.page.localPage}
              data-page-spacer
              aria-hidden="true"
              style:height={`${untrack(() => heightCache.get(pageData.page.localPage, readerWidth))}px`}
            ></div>
          {/if}
        {/each}
      </TooltipProvider>
    </div>

    <span class="sr-only" aria-live="polite">
      {copy.shell.pageOf(visibleLocalPage, initial.pageCount)}
    </span>

    {#if clientMounted && (degradation.loadFailed || degradation.workerDegraded || degradation.apiDegraded || quran.status === "error")}
      <ReaderStatusBanner
        loadFailed={degradation.loadFailed}
        workerDegraded={degradation.workerDegraded}
        apiDegraded={degradation.apiDegraded}
        quranStatusError={quran.status === "error"}
        initialEmpty={initial.ayahs.length === 0}
        failedPage={degradation.failedPage}
        onRetry={retryDegradedPage}
      />
    {/if}

    <ReaderPageNav
      {initial}
      lastLoadedLocalPage={lastLoaded.page.localPage}
      {previousPage}
      {nextPage}
      {previousSurah}
      {nextSurah}
    />
  </div>
</div>

<style>
  .reader-pages {
    overflow-anchor: none;
    outline: none;
  }

  .page-spacer {
    contain: strict;
    overflow-anchor: none;
    pointer-events: none;
  }

  .ayah-list {
    display: flex;
    flex-direction: column;
  }

  /* §22 Bismillah/opener: ceremonial but minimal — 42–48px, margin-block 40–52px, no ornament.
     Size tracks the reader's Arabic setting (default 33px → ~45px) and never drops below 42px. */
  .surah-opener {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: max(42px, calc(var(--reader-arabic-size, 33px) * 1.35));
    line-height: 1.9;
    margin-block: 44px;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-page {
    border-bottom: 1px solid var(--reader-divider);
    padding: 2rem 1.25rem;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-page:last-child {
    border-bottom: 0;
  }

  :global([data-reader-mode="reading"]) .reader-pages[data-source-kind="arabic"] .ayah-list {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
    font-family: var(--reader-arabic-family, var(--font-arabic));
    line-height: 2.35;
    word-spacing: 0.14em;
  }

  :global(html[data-reader-last-read="true"]:not([data-reader-hydrated="true"]))
    .reader-stack::before {
    content: "";
    display: block;
    height: 46px;
    flex: 0 0 46px;
  }

  @media (min-width: 640px) {
    :global([data-reader-mode="reading"]) .reader-pages .surah-page {
      padding-inline: 2.25rem;
    }
  }
</style>
