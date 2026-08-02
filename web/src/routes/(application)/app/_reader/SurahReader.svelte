<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { replaceState } from "$app/navigation";
  import { resolve } from "$app/paths";
  import {
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
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import VerseRow from "./VerseRow.svelte";

  let {
    initial,
    previousPage,
    nextPage,
    previousSurah,
    nextSurah,
    onVisiblePage,
  }: {
    initial: SurahLocalPageData;
    previousPage: SurahLocalPageLink | null;
    nextPage: SurahLocalPageLink | null;
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
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
  let lastScrollY = 0;
  let touchY: number | null = null;
  let scrollFrame = 0;
  let suppressScroll = false;
  let virtualShiftPage: number | null = null;
  let positionQueue = Promise.resolve();
  const pageHeights = new SvelteMap<string, SvelteMap<number, number>>();
  const badge = $derived(String(initial.surah.num).padStart(3, "0"));
  const visibleLocalPage = $derived(activeLocalPage ?? initial.page.localPage);
  const virtualFocusPage = $derived(virtualCenterPage ?? visibleLocalPage);
  const firstLoaded = $derived(pages[0]!);
  const lastLoaded = $derived(pages.at(-1)!);
  const renderedPageNumbers = $derived.by(
    () =>
      new Set(
        virtualPageWindow(
          pages.map((page) => page.page.localPage),
          virtualFocusPage,
        ),
      ),
  );
  const previousHref = $derived.by(() => {
    if (firstLoaded.page.localPage === initial.page.localPage && previousPage) {
      return previousPage.href;
    }
    if (firstLoaded.page.localPage > 1) {
      return surahLocalPagePath(initial.surah, firstLoaded.page.localPage - 1);
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
    firstLoaded.page.localPage > 1
      ? `Page ${firstLoaded.page.localPage - 1}`
      : previousSurah?.name,
  );
  const nextLabel = $derived(
    lastLoaded.page.localPage < initial.pageCount
      ? `Page ${lastLoaded.page.localPage + 1}`
      : nextSurah?.name,
  );

  function layoutKey(): string {
    return `${reader.mode}:${reader.arabicSizePx}`;
  }

  function defaultPageHeight(): number {
    const scale = Number.parseFloat(reader.arabicSizePx) / 33;
    return Math.round((reader.isReadingMode ? 720 : 1320) * scale);
  }

  function pageHeight(localPage: number): number {
    return pageHeights.get(layoutKey())?.get(localPage) ?? defaultPageHeight();
  }

  function savePageHeight(localPage: number, height: number): void {
    if (!Number.isFinite(height) || height <= 0) return;
    const key = layoutKey();
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
      const measure = () => savePageHeight(localPage, node.getBoundingClientRect().height);
      const observer = new ResizeObserver(measure);
      observer.observe(node);
      measure();
      return () => observer.disconnect();
    };
  }

  const captureReaderPages: Attachment<HTMLElement> = (node) => {
    readerPages = node;
    return () => {
      if (readerPages === node) readerPages = null;
    };
  };

  function renderedAnchor(): { localPage: number; top: number } | null {
    if (!readerPages) return null;
    const marker = Math.min(window.innerHeight * 0.35, 260);
    const sections = [
      ...readerPages.querySelectorAll<HTMLElement>("[data-page-rendered]"),
    ];
    if (!sections.length) return null;
    let closest = sections[0]!;
    let distance = Number.POSITIVE_INFINITY;
    for (const section of sections) {
      const rect = section.getBoundingClientRect();
      if (rect.top <= marker && rect.bottom > marker) {
        closest = section;
        break;
      }
      const nextDistance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
      if (nextDistance < distance) {
        closest = section;
        distance = nextDistance;
      }
    }
    return {
      localPage: Number(closest.dataset.localPage),
      top: closest.getBoundingClientRect().top,
    };
  }

  function preserveViewport(change: () => void): Promise<void> {
    const operation = async () => {
      const anchor = renderedAnchor();
      suppressScroll = true;
      try {
        change();
        await tick();
        if (anchor && readerPages) {
          const node = readerPages.querySelector<HTMLElement>(
            `[data-page-rendered][data-local-page="${anchor.localPage}"]`,
          );
          if (node) {
            const delta = node.getBoundingClientRect().top - anchor.top;
            if (Math.abs(delta) > 0.5) window.scrollTo(0, window.scrollY + delta);
          }
        }
        lastScrollY = window.scrollY;
        await new Promise<void>((resolveFrame) => requestAnimationFrame(() => resolveFrame()));
      } finally {
        suppressScroll = false;
      }
    };
    const result = positionQueue.then(operation, operation);
    positionQueue = result.catch(() => undefined);
    return result;
  }

  function shiftVirtualWindow(localPage: number): void {
    if (renderedPageNumbers.has(localPage) || virtualShiftPage === localPage) return;
    virtualShiftPage = localPage;
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
      const page = Number(spacer.dataset.localPage);
      const rect = spacer.getBoundingClientRect();
      return (
        rect.bottom > -900 &&
        rect.top < window.innerHeight + 900 &&
        (direction > 0 ? page > virtualFocusPage : page < virtualFocusPage)
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
    });
  }

  function changeFontSize(change: () => void): void {
    void preserveViewport(() => {
      virtualCenterPage = visibleLocalPage;
      change();
    });
  }

  function cachePage(pageData: SurahLocalPageData): void {
    reader.seedAyahs(
      pageData.ayahs.map((ayah) => ({
        key: ayah.key,
        text: bodyText(ayah.text, ayah.ayah, pageData.normalization),
      })),
    );
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
    if (!quranWorker.ready) {
      pendingPages.add(localPage);
      return;
    }
    pendingPages.delete(localPage);
    loadingPages.add(localPage);
    loadFailed = false;
    try {
      const quranData = await loadQuranData();
      const page = quranData.surahLocalPage(initial.surah.num, localPage);
      if (!page) throw new Error(`Unknown Surah page ${initial.surah.num}:${localPage}`);
      const range = await quranWorker.readRange(
        page.startGlobal,
        page.endGlobal,
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
        page,
        pageCount: initial.pageCount,
        ayahs: range.ayahs,
        normalization,
      };
      cachePage(pageData);
      if (localPage < firstLoaded.page.localPage) {
        await preserveViewport(() => {
          loadedPages = [...loadedPages, pageData];
        });
      } else {
        loadedPages = [...loadedPages, pageData];
        await tick();
      }
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

  function setVisiblePage(localPage: number): void {
    if (!renderedPageNumbers.has(localPage)) shiftVirtualWindow(localPage);
    if (localPage === visibleLocalPage) return;
    activeLocalPage = localPage;
    replaceState(resolve(surahLocalPagePath(initial.surah, localPage)), {});
    const pageData = pages.find((item) => item.page.localPage === localPage);
    if (pageData) onVisiblePage?.(pageData);
  }

  function updateVisiblePage(): void {
    if (!readerPages) return;
    const marker = Math.min(window.innerHeight * 0.35, 260);
    const sections = [...readerPages.querySelectorAll<HTMLElement>("[data-local-page]")];
    if (!sections.length) return;
    let closest = sections[0]!;
    let closestDistance = Number.POSITIVE_INFINITY;
    for (const section of sections) {
      const rect = section.getBoundingClientRect();
      if (rect.top <= marker && rect.bottom > marker) {
        closest = section;
        break;
      }
      const distance = Math.min(Math.abs(rect.top - marker), Math.abs(rect.bottom - marker));
      if (distance < closestDistance) {
        closest = section;
        closestDistance = distance;
      }
    }
    const localPage = Number(closest.dataset.localPage);
    if (Number.isSafeInteger(localPage)) setVisiblePage(localPage);
  }

  function processScroll(direction: number): void {
    scrollFrame = 0;
    warmVirtualWindow(direction);
    updateVisiblePage();
    if (!readerPages) return;
    const rect = readerPages.getBoundingClientRect();
    if (direction < 0 && rect.top > -900) requestPreviousPage();
    if (direction > 0 && rect.bottom - window.innerHeight < 900) requestNextPage();
  }

  function onScroll(): void {
    const currentY = window.scrollY;
    if (suppressScroll) {
      lastScrollY = currentY;
      return;
    }
    const direction = Math.sign(currentY - lastScrollY);
    lastScrollY = currentY;
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
    scrollFrame = requestAnimationFrame(() => processScroll(direction));
  }

  function onWheel(event: WheelEvent): void {
    if (event.deltaY !== 0) processScroll(Math.sign(event.deltaY));
  }

  function onTouchStart(event: TouchEvent): void {
    touchY = event.touches[0]?.clientY ?? null;
  }

  function onTouchMove(event: TouchEvent): void {
    const nextY = event.touches[0]?.clientY;
    if (touchY === null || nextY === undefined) return;
    const direction = Math.sign(touchY - nextY);
    touchY = nextY;
    if (direction !== 0) processScroll(direction);
  }

  function onTouchEnd(): void {
    touchY = null;
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

  function continueReading(): void {
    const lastRead = reader.lastRead;
    if (lastRead) reader.openVerse(lastRead.num, lastRead.n);
  }

  onMount(() => {
    clientMounted = true;
    lastScrollY = window.scrollY;
    cachePage(initial);
    const stop = quranWorker.onStatus((status) => {
      if (status !== "ready") return;
      for (const localPage of [...pendingPages].sort((a, b) => a - b)) {
        void loadPage(localPage);
      }
    });
    return () => {
      stop();
      if (scrollFrame) cancelAnimationFrame(scrollFrame);
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
/>

<div class="flex flex-col gap-4">
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
      class="flex flex-wrap items-start justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:px-9"
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
            <span dir="rtl" class="font-arabic text-[30px] leading-none text-fg-2">
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

    <div {@attach captureReaderPages} class="reader-pages" data-reader-mode={reader.mode}>
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
                <p dir="rtl" class="surah-opener py-3 text-center font-arabic text-fg-3">
                  {headerText(pageData.normalization)}
                </p>
              {/if}
              <ol class="ayah-list list-none p-0">
                {#each pageData.ayahs as ayah (ayah.key)}
                  <VerseRow
                    text={bodyText(ayah.text, ayah.ayah, pageData.normalization)}
                    n={ayah.ayah}
                    vKey={ayah.key}
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

    <span class="sr-only" aria-live="polite">
      {loadFailed ? "An adjacent page could not be loaded. Use the page links to continue." : ""}
    </span>

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

  @media (min-width: 640px) {
    .reader-pages[data-reader-mode="reading"] .surah-page {
      padding-inline: 2.25rem;
    }
  }
</style>
