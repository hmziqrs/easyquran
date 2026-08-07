<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { SvelteSet } from "svelte/reactivity";
  import { beforeNavigate, goto, replaceState } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page as appPage } from "$app/state";
  import {
    parseKey,
    surahAyahPathFor,
    surahLocalPagePathFor,
    surahRouteContext,
    type SurahLocalPageData,
    type SurahLocalPageLink,
    type SurahLink,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { Icon } from "$lib/components/icon";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import { quranWorker } from "$lib/quran/worker-client";
  import { virtualPageWindow } from "$lib/quran/virtual-pages";
  import { bodyText } from "$lib/quran/view/source-view";
  import { headerText } from "$lib/quran/view/presentation";
  import { quran } from "$lib/stores/quran.svelte";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import { PREPARE_RELOAD, PREPARE_RELOAD_EVENT, UPDATE_BROADCAST_CHANNEL } from "$lib/offline/messages";
  import { online } from "$lib/offline/online.svelte";
  import { PageHeightCache, stablePageHeight, widthBucket } from "./page-heights";
  import {
    currentUrlLocalPage,
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
  import VerseRow from "./VerseRow.svelte";

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

  let loadedPages = $state.raw<SurahLocalPageData[]>([]);
  const pages = $derived.by(() => {
    const byPage = new Map<number, SurahLocalPageData>();
    for (const pageData of [initial, ...loadedPages]) {
      const existing = byPage.get(pageData.page.localPage);
      if (!existing || pageData.ayahs.length > 0) byPage.set(pageData.page.localPage, pageData);
    }
    return [...byPage.values()].sort((a, b) => a.page.localPage - b.page.localPage);
  });
  let readerPages: HTMLElement | null = $state(null);
  const pendingPages = new SvelteSet<number>();
  const loadingPages = new SvelteSet<number>();
  let loadFailed = $state(false);
  let clientMounted = $state(false);
  let activeLocalPage = $state<number | null>(null);
  let virtualCenterPage = $state<number | null>(null);
  let readerWidth = $state(0);
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
  const visibleLocalPage = $derived(activeLocalPage ?? initial.page.localPage);
  const virtualFocusPage = $derived(virtualCenterPage ?? visibleLocalPage);
  const firstLoaded = $derived(pages[0]!);
  const lastLoaded = $derived(pages.at(-1)!);
  const sourceId = $derived(initial.normalization.sourceId);
  const routeContext = $derived(surahRouteContext(sourceId));
  const isTranslationSource = $derived(routeContext.kind !== "arabic");
  function pagePathFor(localPage: number): `/app/${string}` {
    return surahLocalPagePathFor(routeContext, initial.surah, localPage);
  }
  const renderedPageNumbers = $derived.by(
    () =>
      new Set(
        virtualPageWindow(
          pages.map((pageData) => pageData.page.localPage),
          virtualFocusPage,
        ),
      ),
  );

  function captureAnchor(): ViewportAnchor | null {
    return captureViewportAnchor(readerPages);
  }

  function restoreAnchor(anchor: ViewportAnchor): void {
    if (restoreViewportAnchor(readerPages, anchor)) lastScrollY = window.scrollY;
  }

  function markAnchorRead(anchor: ViewportAnchor | null | undefined): void {
    if (!userScrolled || anchor?.kind !== "verse") return;
    const { num, n } = parseKey(anchor.verseKey);
    if (num === initial.surah.num) reader.markRead(num, n, sourceId);
  }

  function measurePage(localPage: number): Attachment<HTMLElement> {
    return (node) => {
      let lastTotalHeight = 0;
      const measure = () => {
        const rect = node.getBoundingClientRect();
        const parentWidth = readerPages?.getBoundingClientRect().width ?? rect.width;
        const totalHeight = rect.height;
        heightCache.save(localPage, stablePageHeight(node), parentWidth);
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
          layoutRepairPending = true;
          void preserveViewportFrom(stableAnchor, () => undefined, true).finally(() => {
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
    anchorSource: ViewportAnchor | null | (() => ViewportAnchor | null),
    change: () => void,
    waitForLayout = false,
  ): Promise<void> {
    const operation = async () => {
      const anchor = typeof anchorSource === "function" ? anchorSource() : anchorSource;
      suppressScroll = true;
      try {
        change();
        await tick();
        if (waitForLayout) await nextFrame();
        if (anchor) restoreAnchor(anchor);
        await nextFrame();
        updateVisiblePage();
        stableAnchor = captureAnchor();
      } finally {
        suppressScroll = false;
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
            anchor,
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
    }, true);
  }

  function changeFontSize(change: () => void): void {
    void preserveViewport(() => {
      virtualCenterPage = visibleLocalPage;
      change();
    }, true);
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
    const snapshot = historySnapshot(localPage);
    const target = typeof url === "string" ? url : url.href;
    if (target !== lastWrittenUrl || localPage !== lastWrittenPage) {
      replaceState(url, {
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

  async function restoreHistory(): Promise<void> {
    const saved =
      parseHistoryState(appPage.state.surahReader, initial.surah.num) ??
      reloadPositionState(initial);
    if (!saved) return;
    suppressScroll = true;
    try {
      await restoreHistoryFrom(saved);
      await nextFrame();
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
    if (quran.status === "error") {
      loadFailed = true;
      return;
    }
    if (!quranWorker.ready) {
      pendingPages.add(localPage);
      return;
    }
    pendingPages.delete(localPage);
    loadingPages.add(localPage);
    loadFailed = false;
    try {
      const quranData = await loadQuranData();
      const pageDataRange = quranData.surahLocalPage(initial.surah.num, localPage);
      if (!pageDataRange) {
        throw new Error(`Unknown Surah page ${initial.surah.num}:${localPage}`);
      }
      const range = await quranWorker.readRange(
        pageDataRange.startGlobal,
        pageDataRange.endGlobal,
        (globalIndex, surah, ayah) => quranData.globalIndexOf(surah, ayah) === globalIndex,
        isTranslationSource ? sourceId : undefined,
      );
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
      if (clientMounted) writeHistoryState();
    } catch {
      loadFailed = true;
    } finally {
      loadingPages.delete(localPage);
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
    writeHistoryState(resolve(pagePathFor(localPage)), localPage);
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
    scheduleForwardFill();
  }

  function onKeyDown(event: KeyboardEvent): void {
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
    const lastRead = reader.lastRead;
    if (!lastRead) return;
    try {
      const quranData = await loadQuranData();
      const surah = quranData.surahByNum(lastRead.num);
      const targetPage = quranData.surahLocalPageForAyah(lastRead.num, lastRead.n);
      if (!surah || !targetPage) return;
      const resumeCtx = lastRead.sourceId ? surahRouteContext(lastRead.sourceId) : routeContext;
      reader.openVerse(lastRead.num, lastRead.n, lastRead.sourceId);
      await goto(resolve(surahAyahPathFor(resumeCtx, surah, targetPage.localPage, lastRead.n)), {
        keepFocus: true,
      });
    } catch {
      loadFailed = true;
    }
  }

  beforeNavigate(() => {
    if (!clientMounted) return;
    if (historyWriteTimer) {
      clearTimeout(historyWriteTimer);
      historyWriteTimer = null;
    }
    writeHistoryState();
  });

  onMount(() => {
    clientMounted = true;
    lastScrollY = window.scrollY;
    cachePage(initial);
    if (initial.ayahs.length === 0) void loadPage(initial.page.localPage);
    void restoreHistory().then(() => {
      stableAnchor = captureAnchor();
      scheduleForwardFill();
      void nextFrame().then(() => writeHistoryState());
    });
    const stop = quranWorker.onStatus((status) => {
      if (status === "error") {
        loadFailed = true;
        pendingPages.clear();
        return;
      }
      if (status !== "ready") return;
      for (const localPage of [...pendingPages].sort((a, b) => a - b)) {
        void loadPage(localPage);
      }
      scheduleForwardFill();
    });
    let updateChannel: BroadcastChannel | null = null;
    if (typeof BroadcastChannel !== "undefined") {
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
  {#if reader.hasLastRead}
    <button
      type="button"
      onclick={continueReading}
      class="flex items-center gap-3 rounded-[12px] bg-accent-soft px-[18px] py-[13px] text-left transition-[filter] duration-150 hover:brightness-[0.98]"
    >
      <Icon name="play" size={15} class="flex-none text-accent" />
      <span class="text-sm text-accent">Continue reading — {reader.lastReadRef}</span>
      <span class="ml-auto text-[13px] text-accent/75">Jump →</span>
    </button>
  {/if}

  <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
    <ReaderHeader
      {initial}
      {visibleLocalPage}
      {clientMounted}
      onChangeMode={changeMode}
      onSmaller={() => changeFontSize(() => reader.smaller())}
      onBigger={() => changeFontSize(() => reader.bigger())}
    />

    <div {@attach captureReaderPages} class="reader-pages" tabindex="-1">
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
                {initial.surah.name}, page {pageData.page.localPage} of {initial.pageCount}
              </h2>
              {#if pageData.page.startAyah === 1 && headerText(pageData.normalization)}
                <p dir="rtl" lang="ar" class="surah-opener py-3 text-center font-arabic text-fg-3">
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
                  />
                {/each}
              </ol>
            </section>
          {:else}
            <div
              class="page-spacer"
              data-local-page={pageData.page.localPage}
              data-page-spacer
              aria-hidden="true"
              style:height={`${heightCache.get(pageData.page.localPage, readerWidth)}px`}
            ></div>
          {/if}
        {/each}
      </TooltipProvider>
    </div>

    <span class="sr-only" aria-live="polite">Page {visibleLocalPage} of {initial.pageCount}</span>

    {#if clientMounted && (loadFailed || quran.status === "error")}
      <div
        role="status"
        class="flex items-center justify-between gap-3 border-t border-line bg-bg-2 px-5 py-3 text-sm text-fg-2 sm:px-9"
      >
        <span>
          {#if initial.ayahs.length === 0}
            This translation couldn't be loaded right now.
          {:else if online.hydrated && !online.online}
            You're offline — more ayahs will load when you reconnect. You can keep reading this page or
            use the page links.
          {:else}
            More ayahs are unavailable right now. You can keep reading this page or use the page links.
          {/if}
        </span>
        {#if initial.ayahs.length === 0}
          <button
            type="button"
            onclick={() => {
              loadFailed = false;
              void loadPage(initial.page.localPage);
            }}
            class="shrink-0 rounded-full border border-line px-3 py-1 text-xs text-fg transition-colors hover:bg-bg-1"
          >
            Retry
          </button>
        {/if}
      </div>
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

  :global([data-reader-mode="reading"]) .reader-pages .surah-page {
    border-bottom: 1px solid var(--line);
    padding: 2rem 1.25rem;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-page:last-child {
    border-bottom: 0;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-opener {
    padding-top: 0;
  }

  :global([data-reader-mode="reading"]) .reader-pages .ayah-list {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
    font-family: var(--font-arabic);
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
