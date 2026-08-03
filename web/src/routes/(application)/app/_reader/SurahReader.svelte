<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { beforeNavigate, goto, replaceState } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page as appPage } from "$app/state";
  import {
    parseKey,
    surahAyahPath,
    surahLocalPagePath,
    surahMeta,
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
  import VerseRow from "./VerseRow.svelte";

  type ViewportAnchor =
    | {
        kind: "verse";
        localPage: number;
        verseKey: string;
        viewportPoint: number;
        ratio: number;
      }
    | {
        kind: "page";
        localPage: number;
        viewportPoint: number;
        ratio: number;
      };

  interface SurahReaderHistoryState {
    version: 1;
    surahNum: number;
    activeLocalPage: number;
    pages: SurahLocalPageData[];
    anchor: ViewportAnchor | null;
  }

  const readerPositionKey = "easyquran.reader-position";

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
  const pages = $derived.by(() =>
    [initial, ...loadedPages].sort((a, b) => a.page.localPage - b.page.localPage),
  );
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
  const pageHeights = new SvelteMap<string, SvelteMap<number, number>>();
  const loadAheadPx = 900;
  const badge = $derived(String(initial.surah.num).padStart(3, "0"));
  const visibleLocalPage = $derived(activeLocalPage ?? initial.page.localPage);
  const virtualFocusPage = $derived(virtualCenterPage ?? visibleLocalPage);
  const firstLoaded = $derived(pages[0]!);
  const lastLoaded = $derived(pages.at(-1)!);
  const renderedPageNumbers = $derived.by(
    () =>
      new Set(
        virtualPageWindow(
          pages.map((pageData) => pageData.page.localPage),
          virtualFocusPage,
        ),
      ),
  );
  const previousHref = $derived.by(() => {
    if (lastLoaded.page.localPage === initial.page.localPage && previousPage) {
      return previousPage.href;
    }
    if (lastLoaded.page.localPage > 1) {
      return surahLocalPagePath(initial.surah, lastLoaded.page.localPage - 1);
    }
    return previousSurah ? surahLocalPagePath(previousSurah, 1) : null;
  });
  const nextHref = $derived.by(() => {
    if (lastLoaded.page.localPage === initial.page.localPage && nextPage) {
      return nextPage.href;
    }
    if (lastLoaded.page.localPage < initial.pageCount) {
      return surahLocalPagePath(initial.surah, lastLoaded.page.localPage + 1);
    }
    return nextSurah ? surahLocalPagePath(nextSurah, 1) : null;
  });
  const previousLabel = $derived(
    lastLoaded.page.localPage > 1
      ? `Page ${lastLoaded.page.localPage - 1}`
      : previousSurah?.name,
  );
  const nextLabel = $derived(
    lastLoaded.page.localPage < initial.pageCount
      ? `Page ${lastLoaded.page.localPage + 1}`
      : nextSurah?.name,
  );

  function viewportMarker(): number {
    return Math.min(window.innerHeight * 0.35, 260);
  }

  function widthBucket(width = readerWidth): number {
    return Math.max(320, Math.round(Math.max(width, 320) / 24) * 24);
  }

  function layoutKey(width = readerWidth): string {
    return `${reader.mode}:${reader.arabicSizePx}:${widthBucket(width)}`;
  }

  function defaultPageHeight(): number {
    const fontScale = Number.parseFloat(reader.arabicSizePx) / 33;
    const widthScale = Math.min(2.3, Math.max(0.85, Math.sqrt(1050 / widthBucket())));
    return Math.round((reader.isReadingMode ? 720 : 1320) * fontScale * widthScale);
  }

  function pageHeight(localPage: number): number {
    return pageHeights.get(layoutKey())?.get(localPage) ?? defaultPageHeight();
  }

  function stablePageHeight(node: HTMLElement): number {
    let height = node.getBoundingClientRect().height;
    for (const note of node.querySelectorAll<HTMLElement>(".verse-note")) {
      const style = getComputedStyle(note);
      height -=
        note.getBoundingClientRect().height +
        Number.parseFloat(style.marginTop || "0") +
        Number.parseFloat(style.marginBottom || "0");
    }
    return height;
  }

  function savePageHeight(localPage: number, height: number, width: number): void {
    if (!Number.isFinite(height) || height <= 0) return;
    const key = layoutKey(width);
    let heights = pageHeights.get(key);
    if (!heights) {
      heights = new SvelteMap<number, number>();
      pageHeights.set(key, heights);
    }
    const rounded = Math.ceil(height);
    if (heights.get(localPage) !== rounded) heights.set(localPage, rounded);
  }

  function measurePage(localPage: number): Attachment<HTMLElement> {
    return (node) => {
      let lastTotalHeight = 0;
      const measure = () => {
        const totalHeight = node.getBoundingClientRect().height;
        savePageHeight(
          localPage,
          stablePageHeight(node),
          readerPages?.getBoundingClientRect().width ?? node.getBoundingClientRect().width,
        );
        if (
          lastTotalHeight > 0 &&
          Math.abs(totalHeight - lastTotalHeight) > 1 &&
          clientMounted &&
          !suppressScroll &&
          !layoutRepairPending &&
          stableAnchor &&
          node.getBoundingClientRect().bottom > 0 &&
          node.getBoundingClientRect().top < window.innerHeight
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

  function closestPage(marker: number): HTMLElement | null {
    if (!readerPages) return null;
    const sections = [...readerPages.querySelectorAll<HTMLElement>("[data-local-page]")];
    let closest: HTMLElement | null = null;
    let closestDistance = Number.POSITIVE_INFINITY;
    for (const section of sections) {
      const rect = section.getBoundingClientRect();
      if (rect.top <= marker && rect.bottom > marker) return section;
      const distance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
      if (distance < closestDistance) {
        closest = section;
        closestDistance = distance;
      }
    }
    return closest;
  }

  function captureViewportAnchor(): ViewportAnchor | null {
    const marker = viewportMarker();
    const section = closestPage(marker);
    if (!section) return null;
    const localPage = Number(section.dataset.localPage);
    if (!Number.isSafeInteger(localPage)) return null;
    if (section.hasAttribute("data-page-rendered")) {
      let closestVerse: HTMLElement | null = null;
      let closestDistance = Number.POSITIVE_INFINITY;
      for (const row of section.querySelectorAll<HTMLElement>("[data-verse-key]")) {
        const text = row.querySelector<HTMLElement>(".verse-text");
        if (!text) continue;
        const rect = text.getBoundingClientRect();
        if (rect.top <= marker && rect.bottom > marker) {
          closestVerse = row;
          break;
        }
        const distance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
        if (distance < closestDistance) {
          closestVerse = row;
          closestDistance = distance;
        }
      }
      const text = closestVerse?.querySelector<HTMLElement>(".verse-text");
      const verseKey = closestVerse?.dataset.verseKey;
      if (text && verseKey) {
        const rect = text.getBoundingClientRect();
        return {
          kind: "verse",
          localPage,
          verseKey,
          viewportPoint: marker,
          ratio: Math.min(1, Math.max(0, (marker - rect.top) / Math.max(rect.height, 1))),
        };
      }
    }
    const rect = section.getBoundingClientRect();
    return {
      kind: "page",
      localPage,
      viewportPoint: marker,
      ratio: Math.min(1, Math.max(0, (marker - rect.top) / Math.max(rect.height, 1))),
    };
  }

  function restoreViewportAnchor(anchor: ViewportAnchor): boolean {
    if (!readerPages) return false;
    let node: HTMLElement | null = null;
    if (anchor.kind === "verse") {
      node = readerPages.querySelector<HTMLElement>(
        `[data-verse-key="${anchor.verseKey}"] .verse-text`,
      );
    }
    node ??= readerPages.querySelector<HTMLElement>(
      `[data-local-page="${anchor.localPage}"]`,
    );
    if (!node) return false;
    const rect = node.getBoundingClientRect();
    const targetPoint = rect.top + rect.height * anchor.ratio;
    const delta = targetPoint - anchor.viewportPoint;
    if (Math.abs(delta) > 0.5) window.scrollTo(0, window.scrollY + delta);
    lastScrollY = window.scrollY;
    return true;
  }

  function nextFrame(): Promise<void> {
    return new Promise((resolveFrame) => requestAnimationFrame(() => resolveFrame()));
  }

  function preserveViewportFrom(
    anchorSource: ViewportAnchor | null | (() => ViewportAnchor | null),
    change: () => void,
    waitForLayout = false,
  ): Promise<void> {
    const operation = async () => {
      const anchor =
        typeof anchorSource === "function" ? anchorSource() : anchorSource;
      suppressScroll = true;
      try {
        change();
        await tick();
        if (waitForLayout) await nextFrame();
        if (anchor) restoreViewportAnchor(anchor);
        await nextFrame();
        updateVisiblePage();
        stableAnchor = captureViewportAnchor();
      } finally {
        suppressScroll = false;
      }
    };
    const result = positionQueue.then(operation, operation);
    positionQueue = result.catch(() => undefined);
    return result;
  }

  function preserveViewport(change: () => void, waitForLayout = false): Promise<void> {
    return preserveViewportFrom(captureViewportAnchor, change, waitForLayout);
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
    void preserveViewport(
      () => {
        virtualCenterPage = visibleLocalPage;
        reader.setMode(mode);
      },
      true,
    );
  }

  function changeFontSize(change: () => void): void {
    void preserveViewport(
      () => {
        virtualCenterPage = visibleLocalPage;
        change();
      },
      true,
    );
  }

  function toggleNote(verseKey: string): void {
    void preserveViewport(
      () => {
        reader.toggleNote(verseKey);
      },
      true,
    );
  }

  function cachePage(pageData: SurahLocalPageData): void {
    reader.seedAyahs(
      pageData.ayahs.map((ayah) => ({
        key: ayah.key,
        text: bodyText(ayah.text, ayah.ayah, pageData.normalization),
      })),
    );
  }

  function currentUrlLocalPage(): number {
    const match = /\/page\/(\d+)\/?$/.exec(window.location.pathname);
    return match ? Number(match[1]) : 1;
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
      anchor: captureViewportAnchor(),
    };
  }

  function writeHistoryState(
    url: string | URL = window.location.href,
    localPage = visibleLocalPage,
  ): void {
    const snapshot = historySnapshot(localPage);
    replaceState(url, {
      ...appPage.state,
      surahReader: snapshot,
    });
    try {
      sessionStorage.setItem(
        readerPositionKey,
        JSON.stringify({
          version: snapshot.version,
          surahNum: snapshot.surahNum,
          activeLocalPage: snapshot.activeLocalPage,
          anchor: snapshot.anchor,
        }),
      );
    } catch {}
    if (userScrolled && snapshot.anchor?.kind === "verse") {
      const { num, n } = parseKey(snapshot.anchor.verseKey);
      if (num === initial.surah.num) reader.markRead(num, n);
    }
  }

  function scheduleHistoryWrite(): void {
    if (historyWriteTimer) clearTimeout(historyWriteTimer);
    historyWriteTimer = setTimeout(() => {
      historyWriteTimer = null;
      writeHistoryState();
    }, 180);
  }

  function restoredHistoryState(): SurahReaderHistoryState | null {
    const value = appPage.state.surahReader;
    if (value && typeof value === "object") {
      const state = value as Partial<SurahReaderHistoryState>;
      if (
        state.version === 1 &&
        state.surahNum === initial.surah.num &&
        state.activeLocalPage === currentUrlLocalPage() &&
        Array.isArray(state.pages) &&
        state.pages.some((pageData) => pageData?.page?.localPage === state.activeLocalPage)
      ) {
        return state as SurahReaderHistoryState;
      }
    }
    const navigation = performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
    if (navigation?.type !== "reload") return null;
    try {
      const state = JSON.parse(sessionStorage.getItem(readerPositionKey) ?? "null") as
        | Partial<SurahReaderHistoryState>
        | null;
      if (
        state?.version === 1 &&
        state.surahNum === initial.surah.num &&
        state.activeLocalPage === initial.page.localPage &&
        state.anchor
      ) {
        return {
          version: 1,
          surahNum: initial.surah.num,
          activeLocalPage: initial.page.localPage,
          pages: [initial],
          anchor: state.anchor,
        };
      }
    } catch {}
    return null;
  }

  async function restoreHistory(): Promise<void> {
    const saved = restoredHistoryState();
    if (!saved) return;
    // Restoring scrolls the window, which must not be mistaken for the reader
    // having been scrolled by the user.
    suppressScroll = true;
    try {
      await restoreHistoryFrom(saved);
      await nextFrame();
    } finally {
      suppressScroll = false;
    }
  }

  async function restoreHistoryFrom(saved: SurahReaderHistoryState): Promise<void> {
    const byPage = new SvelteMap<number, SurahLocalPageData>();
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
    if (saved.anchor) restoreViewportAnchor(saved.anchor);
    await document.fonts.ready;
    await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 80));
    if (saved.anchor) restoreViewportAnchor(saved.anchor);
    stableAnchor = captureViewportAnchor();
    updateVisiblePage();
  }

  async function loadPage(localPage: number): Promise<void> {
    if (
      localPage < 1 ||
      localPage > initial.pageCount ||
      pages.some((item) => item.page.localPage === localPage) ||
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
        (globalIndex, surah, ayah) =>
          quranData.globalIndexOf(surah, ayah) === globalIndex,
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

  function setVisiblePage(localPage: number): void {
    if (!renderedPageNumbers.has(localPage)) shiftVirtualWindow(localPage);
    if (localPage === visibleLocalPage) return;
    activeLocalPage = localPage;
    const pageData = pages.find((item) => item.page.localPage === localPage);
    if (pageData) onVisiblePage?.(pageData);
    const anchor = captureViewportAnchor();
    if (userScrolled && anchor?.kind === "verse") {
      const { num, n } = parseKey(anchor.verseKey);
      if (num === initial.surah.num) reader.markRead(num, n);
    }
    writeHistoryState(resolve(surahLocalPagePath(initial.surah, localPage)), localPage);
  }

  function updateVisiblePage(): void {
    const section = closestPage(viewportMarker());
    const localPage = Number(section?.dataset.localPage);
    if (Number.isSafeInteger(localPage)) setVisiblePage(localPage);
  }

  function processScroll(direction: number): void {
    scrollFrame = 0;
    // Anchoring and restores move the window themselves; they must not be
    // treated as reading progress, but load-ahead below still applies.
    if (suppressScroll || anchorScrolling) {
      stableAnchor = captureViewportAnchor();
    } else {
      if (sawUserInput) userScrolled = true;
      updateVisiblePage();
      stableAnchor = captureViewportAnchor();
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
      stableAnchor = captureViewportAnchor();
      // An anchor scroll can land at the end of what is loaded, so keep filling
      // forward — otherwise the target cannot be centred until the user scrolls.
      scheduleForwardFill();
      return;
    }
    const direction = Math.sign(currentY - lastScrollY);
    lastScrollY = currentY;
    if (direction !== 0 && sawUserInput) userScrolled = true;
    warmVirtualWindow(direction);
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
    scrollFrame = requestAnimationFrame(() => processScroll(direction));
  }

  function atScrollBoundary(direction: number): boolean {
    if (direction < 0) return window.scrollY <= 1;
    return window.scrollY >= document.documentElement.scrollHeight - window.innerHeight - 1;
  }

  // Scroll events alone cannot be trusted as "the user read this" — restores,
  // anchoring and SvelteKit's own navigation scrolling all fire them. Marking a
  // verse read requires a real input first.
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
    } else if (
      event.key === "ArrowDown" ||
      event.key === "PageDown" ||
      event.key === "End"
    ) {
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
      reader.openVerse(lastRead.num, lastRead.n);
      await goto(resolve(surahAyahPath(surah, targetPage.localPage, lastRead.n)), {
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
    void restoreHistory().then(() => {
      stableAnchor = captureViewportAnchor();
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
    scheduleForwardFill();
    return () => {
      stop();
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
    <div
      class="flex min-h-[229px] flex-wrap items-start justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:min-h-0 sm:px-9"
    >
      <div class="flex items-start gap-4">
        <div
          aria-hidden="true"
          class="flex h-16 w-16 flex-none items-center justify-center rounded-2xl border border-accent bg-accent-soft font-arabic text-lg text-accent"
        >
          {badge}
        </div>
        <div class="flex min-w-0 flex-col gap-1.5">
          <span class="text-xs font-semibold uppercase tracking-[0.1em] text-accent">
            Surah {initial.surah.num} · Page {visibleLocalPage} of {initial.pageCount}
          </span>
          <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
            <h1 class="text-[32px] font-semibold tracking-[-0.025em]">
              {initial.surah.num}. {initial.surah.name}
            </h1>
            <span dir="rtl" lang="ar" class="font-arabic text-[30px] leading-none text-fg-2">
              {initial.surah.arabic}
            </span>
          </div>
          <span class="text-sm text-fg-3">{surahMeta(initial.surah)}</span>
        </div>
      </div>

      {#if clientMounted}
        <div class="flex flex-wrap items-center justify-end gap-2">
          <div
            class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
            role="group"
            aria-label="Arabic text size"
          >
            <button
              type="button"
              onclick={() => changeFontSize(() => reader.smaller())}
              aria-label="Smaller Arabic text"
              class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
            >
              A&minus;
            </button>
            <button
              type="button"
              onclick={() => changeFontSize(() => reader.bigger())}
              aria-label="Larger Arabic text"
              class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
            >
              A+
            </button>
          </div>

          <div
            class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
            aria-label="Reading mode"
          >
            <button
              type="button"
              aria-pressed={reader.isVerseMode}
              onclick={() => changeMode("verse")}
              class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors aria-pressed:bg-bg-3 aria-pressed:text-fg text-fg-3 hover:text-fg"
            >
              <Icon name="rows" size={13} />
              <span class="hidden sm:inline">Ayah-by-Ayah</span>
              <span class="sm:hidden">Ayahs</span>
            </button>
            <button
              type="button"
              aria-pressed={reader.isReadingMode}
              onclick={() => changeMode("reading")}
              class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors aria-pressed:bg-bg-3 aria-pressed:text-fg text-fg-3 hover:text-fg"
            >
              <Icon name="continuous" size={13} />
              <span>Reading</span>
            </button>
          </div>
        </div>
      {/if}
    </div>

    <div
      {@attach captureReaderPages}
      class="reader-pages"
      data-reader-mode={reader.mode}
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
                {initial.surah.name}, page {pageData.page.localPage} of {initial.pageCount}
              </h2>
              {#if pageData.page.startAyah === 1 && headerText(pageData.normalization)}
                <p
                  dir="rtl"
                  lang="ar"
                  class="surah-opener py-3 text-center font-arabic text-fg-3"
                >
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
              style:height={`${pageHeight(pageData.page.localPage)}px`}
            ></div>
          {/if}
        {/each}
      </TooltipProvider>
    </div>

    <span class="sr-only" aria-live="polite">Page {visibleLocalPage} of {initial.pageCount}</span>

    {#if clientMounted && (loadFailed || quran.status === "error")}
      <div role="status" class="border-t border-line bg-bg-2 px-5 py-3 text-sm text-fg-2 sm:px-9">
        More ayahs are unavailable right now. You can keep reading this page or use the page links.
      </div>
    {/if}

    <nav
      aria-label="Surah pages"
      class="flex items-center justify-between gap-4 border-t border-line px-5 py-[22px] sm:px-9"
    >
      {#if previousHref && previousLabel}
        <a
          href={resolve(previousHref)}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          <span aria-hidden="true">←</span>
          {previousLabel}
        </a>
      {:else}
        <span></span>
      {/if}
      {#if nextHref && nextLabel}
        <a
          href={resolve(nextHref)}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {nextLabel}
          <span aria-hidden="true">→</span>
        </a>
      {/if}
    </nav>
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

  .reader-pages[data-reader-mode="reading"] .surah-page {
    border-bottom: 1px solid var(--line);
    padding: 2rem 1.25rem;
  }

  .reader-pages[data-reader-mode="reading"] .surah-page:last-child {
    border-bottom: 0;
  }

  .reader-pages[data-reader-mode="reading"] .surah-opener {
    padding-top: 0;
  }

  .reader-pages[data-reader-mode="reading"] .ayah-list {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
    font-family: var(--font-arabic);
    line-height: 2.35;
    word-spacing: 0.14em;
  }

  :global(html[data-reader-mode="reading"]) .reader-pages .surah-page {
    border-bottom: 1px solid var(--line);
    padding: 2rem 1.25rem;
  }

  :global(html[data-reader-mode="reading"]) .reader-pages .surah-page:last-child {
    border-bottom: 0;
  }

  :global(html[data-reader-mode="reading"]) .reader-pages .surah-opener {
    padding-top: 0;
  }

  :global(html[data-reader-mode="reading"]) .reader-pages .ayah-list {
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
    .reader-pages[data-reader-mode="reading"] .surah-page {
      padding-inline: 2.25rem;
    }

    :global(html[data-reader-mode="reading"]) .reader-pages .surah-page {
      padding-inline: 2.25rem;
    }
  }
</style>
