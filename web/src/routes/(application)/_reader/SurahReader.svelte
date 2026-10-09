<script lang="ts">
  import { onMount, onDestroy, tick, untrack } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { SvelteSet } from "svelte/reactivity";
  import { beforeNavigate, goto, refreshAll, replaceState } from "$app/navigation";
  import { page as appPage } from "$app/state";
  import {
    parseKey,
    surahRouteContext,
    type MushafPageLink,
    type SurahLocalPageData,
    type SurahLink,
  } from "#lib/data/quran.js";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import { positionForGlobal } from "#lib/data/mushaf-divisions.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { TooltipProvider } from "#lib/components/ui/tooltip/index.js";
  import { quranWorker } from "#lib/quran/worker-client.js";
  import { TRANSLATION_CATALOGUE, TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import type { ReadTierStatus } from "#lib/quran/fetch.js";
  import {
    virtualPageWindow,
  } from "#lib/quran/virtual-pages.js";
  import { bodyText } from "#lib/quran/view/source-view.js";
  import { headerText } from "#lib/quran/view/presentation.js";
  // Direct import: the brand barrel pulls Brand -> config/site -> $app/env/public,
  // which must not enter this module graph.
  import Bismillah from "#lib/components/brand/Bismillah.svelte";
  import { quran } from "#lib/stores/quran.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { stickyNav } from "#lib/stores/sticky-nav.svelte.js";
  import { withModeParam } from "#lib/reader/mode-param.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { readingText } from "#lib/stores/reading-text.svelte.js";
  import { PREPARE_RELOAD, PREPARE_RELOAD_EVENT, UPDATE_BROADCAST_CHANNEL } from "#lib/offline/messages.js";
  import { ayahIndexValidator } from "./range-validate";
  import {
    parseHistoryState,
    persistReaderPosition,
    reloadPositionState,
    type SurahReaderHistoryState,
  } from "./reader-history";
  import {
    captureViewportAnchor,
    nextFrame,
    restoreViewportAnchor,
    viewportMarker,
    type ViewportAnchor,
  } from "./viewport-anchor";
  import ReaderHeader from "./ReaderHeader.svelte";
  import ReaderPageNav from "./ReaderPageNav.svelte";
  import ReaderStatusBanner from "./ReaderStatusBanner.svelte";
  import { readingFlowId, readingQuickPicks, type ReadPick } from "./reading-flow";
  import TranslationModal from "./TranslationModal.svelte";
  import { registerTypographyWrapper } from "./typography-change";
  import { createArabicCompanion } from "./arabic-companion.svelte";
  import type { Ayah, QuranScript, StackedTranslation } from "#lib/data/quran-types.js";
  import { loadArabicFont } from "#lib/fonts/arabic-fonts.js";
  import { arabicHrefFor, liveReaderPosition } from "./translation-nav";
  import { positionLabel } from "./position-label";
  import { ReaderDegradationState } from "./reader-degradation.svelte";
  import VerseRow from "./VerseRow.svelte";
  import ReadingAyah from "./ReadingAyah.svelte";
  import ReaderVirtualList from "./ReaderVirtualList.svelte";
  import { estimateTextHeight, type ReaderRenderItem } from "./virtual-reader";
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
    previousPage: MushafPageLink | null;
    nextPage: MushafPageLink | null;
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
    anchorScrolling?: boolean;
    onVisiblePage?: (pageData: SurahLocalPageData) => void;
  } = $props();

  const copy = getReaderUiCopy();

  let loadedPages = $state.raw<SurahLocalPageData[]>([]);
  let pageOrigin = $state<number | null>(null);
  const allPages = $derived.by(() => {
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
  const pages = $derived.by(() => {
    const origin = pageOrigin ?? initial?.page.localPage;
    const index = allPages.findIndex((entry) => entry.page.localPage === origin);
    if (index < 0) return initial ? [initial] : [];
    let start = index;
    let end = index;
    while (start > 0 && allPages[start - 1]!.page.localPage === allPages[start]!.page.localPage - 1) start -= 1;
    while (end + 1 < allPages.length && allPages[end + 1]!.page.localPage === allPages[end]!.page.localPage + 1) end += 1;
    return allPages.slice(start, end + 1);
  });
  let readerPages: HTMLElement | null = $state(null);
  const loadingPages = new SvelteSet<number>();
  const degradation = new ReaderDegradationState();
  let initialRetryInFlight = false;
  let clientMounted = $state(false);
  let activeLocalPage = $state<number | null>(null);
  let readerWidth = $state(0);
  let lastScrollY = 0;
  let touchY: number | null = null;
  let scrollFrame = 0;
  let forwardFillFrame = 0;
  let historyWriteTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressScroll = $state(false);
  let sawUserInput = false;
  let userScrolled = false;
  let layoutRepairPending = false;
  let stableAnchor: ViewportAnchor | null = null;
  let positionQueue = Promise.resolve();
  const loadAheadPx = 900;
  const visibleLocalPage = $derived(activeLocalPage ?? initial?.page.localPage ?? 1);
  const visiblePageData = $derived(
    pages.find((item) => item.page.localPage === visibleLocalPage) ?? initial,
  );
  const firstLoaded = $derived(pages[0]!);
  const lastLoaded = $derived(pages.at(-1)!);
  const sourceId = $derived(initial.normalization.sourceId);
  const routeContext = $derived(surahRouteContext(sourceId));
  const isTranslationSource = $derived(routeContext.kind !== "arabic");
  // Arabic reads serve the persisted script preference (docs/quran-system.md):
  // the SSG first paint is uthmani; hydration upgrades pages through the
  // worker/API ladder with the preferred variant corpus.
  const readSourceId = $derived(isTranslationSource ? sourceId : reader.arabicScript);
  const routeKey = $derived(`${readSourceId}:${initial.surah.num}:${initial.page.localPage}`);
  let lastRouteKey: string | null = null;
  let stackedQuranData = $state<Awaited<ReturnType<typeof loadQuranData>> | null>(null);
  const stackedController = createStackedTranslations({
    from: () => readFrom,
    to: () => readTo,
    validator: () => (stackedQuranData ? ayahIndexValidator(stackedQuranData) : null),
    routeSourceId: () => (isTranslationSource ? sourceId : null),
    catalogue: () => TRANSLATION_CATALOGUE,
    routeKey: () => `${sourceId}:${initial.surah.num}`,
    ids: () => {
      if (reader.isVerseMode) return stackedTranslations.ids;
      if (flowFromStack && flowId !== null && stackedTranslations.ids.includes(flowId)) return [flowId];
      return [];
    },
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
        publishPosition(initial);
      })
      .catch(() => {});
  });
  /** Live mushaf position for the sticky indicator (best-effort: needs division data). */
  function publishPosition(pageData: SurahLocalPageData): void {
    const quranData = peekQuranData();
    if (!quranData) return;
    reader.setPosition(positionForGlobal(quranData, pageData.page.startGlobal));
  }

  let virtualList: ReaderVirtualList<ReaderRenderItem> | undefined = $state();
  let renderedItems = $state.raw<readonly ReaderRenderItem[]>([]);
  const renderedPageNumbers = $derived(new Set(renderedItems.map((entry) => entry.localPage)));
  const readPages = $derived.by(() => {
    if (renderedItems.length === 0) return [initial];
    const mountedPages = pages.filter((entry) => renderedPageNumbers.has(entry.page.localPage));
    return mountedPages.length > 0 ? mountedPages : [initial];
  });
  const readFrom = $derived(Math.min(...readPages.map((entry) => entry.page.startGlobal)));
  const readTo = $derived(Math.max(...readPages.map((entry) => entry.page.endGlobal)));
  const renderItems = $derived.by((): ReaderRenderItem[] => {
    const arabicSize = Number.parseFloat(reader.arabicSizePx ?? "33");
    const translationSize = Number.parseFloat(reader.translationSizePx ?? "17");
    if (reader.isReadingMode) {
      return pages.map((pageData) => {
        const text = pageData.ayahs.map((ayah) => rowView(ayah, pageData).text).join(" ");
        const size = showsTranslation ? translationSize : arabicSize;
        const lineHeight = showsTranslation ? 1.9 : 2.35;
        const openerHeight = pageData.page.startAyah === 1 && headerText(pageData.normalization) ? 88 : 0;
        return { kind: "page", key: `page:${pageData.page.localPage}`, localPage: pageData.page.localPage, pageData, estimate: 64 + openerHeight + estimateTextHeight(text, size, readerWidth, lineHeight) };
      });
    }
    return pages.flatMap((pageData) => pageData.ayahs.map((ayah) => {
      const view = rowView(ayah, pageData);
      const lanes = lanesFor(ayah.key, view.lead);
      let estimate = 86 + estimateTextHeight(view.text, arabicSize, readerWidth, 2.15);
      for (const lane of lanes) estimate += 34 + estimateTextHeight(lane.text, translationSize, Math.min(readerWidth, 720), 1.85);
      return { kind: "ayah", key: `${pageData.page.localPage}:${ayah.key}`, verseKey: ayah.key, localPage: pageData.page.localPage, ayah, pageData, estimate };
    }));
  });

  function syncRendered(rendered: readonly ReaderRenderItem[]): void {
    if (rendered.length === renderedItems.length && rendered.every((entry, index) => entry.key === renderedItems[index]?.key)) return;
    renderedItems = rendered;
  }

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

  function repairReaderItem(node: HTMLElement): void {
    const rect = node.getBoundingClientRect();
    if (!clientMounted || suppressScroll || layoutRepairPending || !stableAnchor || rect.bottom <= 0 || rect.top >= window.innerHeight) return;
    const anchor = stableAnchor;
    layoutRepairPending = true;
    void preserveViewportFrom(() => anchor, () => undefined, true).finally(() => {
      layoutRepairPending = false;
    });
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
      // The shared collapsible nav keys off raw scroll deltas; the restore below
      // moves scrollY programmatically, so keep its collapse/expand logic blind
      // for the whole window (plus a short tail) or the header oscillates.
      const releaseNavSuppression = stickyNav.suppressProgrammaticScroll();
      try {
        change();
        await tick();
        if (waitForLayout) await nextFrame();
        if (anchor) {
          await virtualList?.prepareAnchor(anchor);
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
        releaseNavSuppression();
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
      if (nextWidth > 0 && nextWidth !== readerWidth) {
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
            () => undefined,
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

  // Reading flows exactly one text — the Arabic or one translation — and switching between
  // them never asks: Ayah-by-Ayah keeps the Arabic plus every stacked translation, Reading
  // follows the saved readingText choice (see reading-flow.ts). A translation route always
  // flows a translation; its Arabic lives at the Arabic URL.
  const routeTranslationId = $derived(isTranslationSource ? sourceId : null);
  let pageReadingPick = $state<string | null>(null);
  let readingPickerOpen = $state(false);
  const flowId = $derived(
    reader.isReadingMode
      ? readingFlowId(
          readingText.text,
          readingText.translationId,
          routeTranslationId,
          pageReadingPick,
          stackedTranslations.ids,
        )
      : null,
  );
  const renderLayoutKey = $derived(`${reader.mode}:${readerWidth}:${reader.arabicFont}:${reader.arabicSizePx}:${reader.arabicScript}:${reader.translationSizePx}:${reader.translationFamily}:${sourceId}:${flowId}:${stackedTranslations.ids.join(",")}`);
  // The route's own translation is the page text itself; any other flows from stacked data.
  const flowFromStack = $derived(flowId !== null && flowId !== routeTranslationId);
  const flowLanguage = $derived(
    flowId === null ? undefined : TRANSLATION_CATALOGUE_BY_ID.get(flowId)?.languageCode,
  );
  const showsTranslation = $derived(isTranslationSource || flowId !== null);
  // A flowing Urdu (or other RTL) translation needs an RTL paragraph, or its ayah markers
  // land on the wrong side of each run.
  const flowRtl = $derived(
    flowId !== null && TRANSLATION_CATALOGUE_BY_ID.get(flowId)?.direction === "rtl",
  );
  const flowName = $derived(
    flowId === null ? null : (TRANSLATION_CATALOGUE_BY_ID.get(flowId)?.name ?? null),
  );
  // Reading can flow any catalogue translation. A stacked one is already fetched with the
  // stack; anything else gets its own one-id fetcher, so it never shows up as a stacked lane.
  const readingFetchIds = $derived(
    flowFromStack && flowId !== null && !stackedTranslations.ids.includes(flowId) ? [flowId] : [],
  );
  const readingController = createStackedTranslations({
    from: () => readFrom,
    to: () => readTo,
    validator: () => (stackedQuranData ? ayahIndexValidator(stackedQuranData) : null),
    routeSourceId: () => (isTranslationSource ? sourceId : null),
    catalogue: () => TRANSLATION_CATALOGUE,
    routeKey: () => `${sourceId}:${initial.surah.num}`,
    ids: () => readingFetchIds,
  });
  $effect(() => readingController.sync());
  onDestroy(() => readingController.dispose());

  // Ayah-by-Ayah never drops the Arabic: on a translation route it is fetched alongside, and
  // the route's own translation becomes the first lane above the stacked ones.
  const arabicCompanion = createArabicCompanion({
    enabled: () => isTranslationSource && reader.isVerseMode,
    source: () => reader.arabicScript,
    from: () => readFrom,
    to: () => readTo,
    validator: () => (stackedQuranData ? ayahIndexValidator(stackedQuranData) : null),
    routeKey: () => `${sourceId}:${initial.surah.num}`,
  });
  $effect(() => arabicCompanion.sync());
  onDestroy(() => arabicCompanion.dispose());
  const routeEntry = $derived(
    routeTranslationId === null ? undefined : TRANSLATION_CATALOGUE_BY_ID.get(routeTranslationId),
  );

  interface RowView {
    readonly text: string;
    readonly isTranslation: boolean | undefined;
    readonly translationLang: string | undefined;
    readonly pending: boolean;
    readonly arabicPending: boolean;
    readonly script: QuranScript;
    readonly lead: StackedTranslation | null;
  }

  /** What one ayah row shows: Arabic + lanes, a flowed translation, or the page text. */
  function rowView(ayah: Ayah, pageData: SurahLocalPageData): RowView {
    const own = bodyText(ayah.text, ayah.ayah, pageData.normalization);
    if (isTranslationSource && reader.isVerseMode && routeEntry) {
      const arabic = arabicCompanion.state.byKey.get(ayah.key);
      return {
        text: arabic?.text ?? "",
        isTranslation: false,
        translationLang: undefined,
        pending: false,
        arabicPending: arabic === undefined,
        script: arabic?.script ?? pageData.normalization.script,
        lead: {
          sourceId: routeEntry.id,
          name: routeEntry.name,
          translator: routeEntry.translator,
          language: routeEntry.language,
          languageCode: routeEntry.languageCode,
          direction: routeEntry.direction,
          text: own,
        },
      };
    }
    if (flowFromStack) {
      const flowed = flowText(ayah.key);
      return {
        text: flowed ?? "",
        isTranslation: true,
        translationLang: flowLanguage,
        pending: flowed === null,
        arabicPending: false,
        script: pageData.normalization.script,
        lead: null,
      };
    }
    return {
      text: own,
      isTranslation: undefined,
      translationLang: undefined,
      pending: false,
      arabicPending: false,
      script: pageData.normalization.script,
      lead: null,
    };
  }

  function lanesFor(key: string, lead: StackedTranslation | null): readonly StackedTranslation[] {
    const stacked = stackedFor(stackedController.state, key);
    return lead ? [lead, ...stacked] : stacked;
  }

  function flowText(key: string): string | null {
    const fromStack = stackedFor(stackedController.state, key).find((t) => t.sourceId === flowId);
    if (fromStack) return fromStack.text;
    return stackedFor(readingController.state, key).find((t) => t.sourceId === flowId)?.text ?? null;
  }

  const readPick = $derived<ReadPick>({
    current: flowId,
    quick: readingQuickPicks(routeTranslationId, readingText.recent, stackedTranslations.ids),
    onPick: pickReadingTranslation,
  });

  function readArabic(): void {
    if (!isTranslationSource) {
      void preserveViewport(() => readingText.readArabic(), true);
      return;
    }
    // A translation route carries no Arabic text: open the Arabic URL at the same place.
    readingText.readArabic();
    const href = arabicHrefFor(liveReaderPosition(appPage.url));
    if (href === null) return;
    const target = withModeParam(publicHref(readerHrefFor(copy.locale, href)), "reading", appPage.url);
    void goto(target, { reset: false });
  }

  /** Translation pill: flow the last translation, or open the picker when there is none. */
  function readTranslation(): void {
    const next = readingText.translationId ?? stackedTranslations.ids[0] ?? null;
    if (next === null) {
      readingPickerOpen = true;
      return;
    }
    void preserveViewport(() => readingText.readTranslation(next), true);
  }

  function pickReadingTranslation(id: string): void {
    void preserveViewport(() => {
      if (isTranslationSource) pageReadingPick = id;
      readingText.readTranslation(id);
    }, true);
  }

  // Urdu and other Arabic-script translations read in Naskh; load it once one is on screen.
  const visibleTranslationIds = $derived(reader.isVerseMode ? [...stackedTranslations.ids, routeTranslationId] : [flowId]);
  const needsNaskh = $derived(
    visibleTranslationIds.some(
      (id) => id !== null && TRANSLATION_CATALOGUE_BY_ID.get(id)?.direction === "rtl",
    ),
  );
  $effect(() => {
    if (needsNaskh) void loadArabicFont("noto-naskh-arabic");
  });

  // The sticky bar's A−/A+ route through this so a resize keeps the reading position.
  $effect(() => registerTypographyWrapper(changeTypography));

  function changeTypography(change: () => void): void {
    void preserveViewport(change, true);
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

  function writeHistoryState(localPage = visibleLocalPage): void {
    if (!initial || anchorScrolling) return;
    const snapshot = historySnapshot(localPage);
    // One URL per surah: the path never moves, so the restore snapshot rides in
    // history.state on the current path and the settled verse position rides in
    // the ?v={surah}:{ayah} query (share/reload parity, quran.com's startingVerse).
    const base = new URL(window.location.href);
    const params = new URLSearchParams(base.search);
    params.delete("v");
    params.delete("verse");
    const anchor = snapshot.anchor;
    if (anchor !== null && anchor.kind === "verse") params.set("v", anchor.verseKey);
    const query = params.toString();
    const bare = `${base.pathname}${query === "" ? "" : `?${query}`}`;
    const next = withModeParam(bare, reader.mode, window.location.href);
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
      const releaseNavSuppression = stickyNav.suppressProgrammaticScroll();
      try {
        await restoreHistoryFrom(saved);
        await nextFrame();
      } finally {
        suppressScroll = false;
        releaseNavSuppression();
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
      const releaseNavSuppression = stickyNav.suppressProgrammaticScroll();
      try {
        await nextFrame();
        await nextFrame();
        await virtualList?.prepareAnchor(anchor);
        restoreAnchor(anchor);
        await document.fonts.ready;
        await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 80));
        restoreAnchor(anchor);
        stableAnchor = captureAnchor();
      } finally {
        suppressScroll = false;
        releaseNavSuppression();
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
    pageOrigin = saved.activeLocalPage;
    activeLocalPage = saved.activeLocalPage;
    for (const pageData of saved.pages) cachePage(pageData);
    await tick();
    onVisiblePage?.(
      saved.pages.find((pageData) => pageData.page.localPage === saved.activeLocalPage) ?? initial,
    );
    await nextFrame();
    await nextFrame();
    if (saved.anchor) {
      await virtualList?.prepareAnchor(saved.anchor);
      restoreAnchor(saved.anchor);
    }
    await document.fonts.ready;
    await new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 80));
    if (saved.anchor) {
      await virtualList?.prepareAnchor(saved.anchor);
      restoreAnchor(saved.anchor);
    }
    stableAnchor = captureAnchor();
    updateVisiblePage();
  }

  async function loadPage(localPage: number): Promise<void> {
    if (
      localPage < 1 ||
      localPage > initial.pageCount ||
      allPages.some(
        (item) =>
          item.page.localPage === localPage &&
          item.ayahs.length > 0 &&
          item.normalization.sourceId === readSourceId,
      ) ||
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
        readSourceId,
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

  export async function ensureAyah(localPage: number, verseKey: string | null): Promise<void> {
    if (!initial || localPage < 1 || localPage > initial.pageCount) return;
    const expectedRoute = routeKey;
    const available = () => allPages.some((entry) => entry.page.localPage === localPage && entry.ayahs.length > 0);
    if (!available()) await loadPage(localPage);
    if (!available() && !quranWorker.ready) {
      await quranWorker.whenReady().catch(() => undefined);
      if (!initial || expectedRoute !== routeKey) return;
      await loadPage(localPage);
    }
    if (!available() || !initial || expectedRoute !== routeKey) return;
    pageOrigin = localPage;
    await tick();
    await virtualList?.reveal(verseKey, localPage);
  }

  export async function ensurePage(localPage: number): Promise<void> {
    await ensureAyah(localPage, null);
  }

  async function retryInitialPage(): Promise<void> {
    if (initialRetryInFlight) return;
    initialRetryInFlight = true;
    const startKey = routeKey;
    try {
      await refreshAll();
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
    if (localPage === visibleLocalPage) return;
    activeLocalPage = localPage;
    const pageData = pages.find((item) => item.page.localPage === localPage);
    if (pageData) {
      onVisiblePage?.(pageData);
      publishPosition(pageData);
    }
    markAnchorRead(anchor !== undefined ? anchor : captureAnchor());
    // Path never moves — only the history-state snapshot and ?v= query update.
    writeHistoryState(localPage);
  }

  function updateVisiblePage(anchor: ViewportAnchor | null = null): void {
    if (anchor) {
      setVisiblePage(anchor.localPage, anchor);
      return;
    }
    const current = captureAnchor();
    if (current) setVisiblePage(current.localPage, current);
  }

  function processScroll(direction: number): void {
    scrollFrame = 0;
    if (suppressScroll || anchorScrolling) {
      stableAnchor = captureAnchor();
    } else {
      if (sawUserInput) userScrolled = true;
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

  // Script switch: drop pages served from the old corpus and re-request the
  // visible window from the preferred variant (worker stages the artifact on
  // demand; the API ladder covers the first cold read).
  let appliedScript: string | null = null;
  $effect(() => {
    const script = reader.arabicScript;
    const previous = appliedScript;
    appliedScript = script;
    if (!clientMounted || isTranslationSource || previous === null || previous === script) return;
    const wanted = untrack(() => pages.map((pageData) => pageData.page.localPage));
    void preserveViewport(
      () => {
        loadedPages = loadedPages.filter(
          (pageData) => pageData.normalization.sourceId === script,
        );
      },
      true,
    );
    for (const localPage of wanted) void loadPage(localPage);
  });

  let lastTypography: string | null = null;
  $effect(() => {
    const typography = `${reader.arabicFont}:${reader.arabicSizePx}:${reader.translationSizePx}:${reader.translationFamily}`;
    if (lastTypography === typography) return;
    const firstRun = lastTypography === null;
    lastTypography = typography;
    if (firstRun) return;
    void preserveViewport(() => undefined, true);
  });

  onMount(() => {
    clientMounted = true;
    lastScrollY = window.scrollY;
    cachePage(initial);
    if (initial.ayahs.length === 0) void retryInitialPage();
    else if (!isTranslationSource && reader.arabicScript !== initial.normalization.sourceId)
      void loadPage(initial.page.localPage);
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


{#snippet renderReaderItem(entry: ReaderRenderItem, index: number, gap: number, measure: Attachment<HTMLElement>)}
  {#if entry.kind === "ayah"}
    {@const view = rowView(entry.ayah, entry.pageData)}
    <VerseRow
      text={view.text}
      isTranslation={view.isTranslation}
      translationLang={view.translationLang}
      pending={view.pending}
      arabicPending={view.arabicPending}
      leadId={view.lead?.sourceId}
      n={entry.ayah.ayah}
      vKey={entry.ayah.key}
      script={view.script}
      localPage={entry.localPage}
      virtualIndex={index}
      totalAyahs={initial.surah.ayahCount}
      virtualGap={gap}
      {measure}
      stacked={lanesFor(entry.ayah.key, view.lead)}
      stackedPending={loadingFor(stackedController.state, entry.ayah.key)}
      stackedErrored={erroredFor(stackedController.state, entry.ayah.key)}
      stackedErrorLabel={copy.stacked.error}
    />
  {:else}
    <section
      class="surah-page"
      data-local-page={entry.localPage}
      data-page-rendered
      data-index={index}
      data-last-page={entry.localPage === initial.pageCount || undefined}
      aria-labelledby="surah-page-{entry.localPage}-title"
      style:margin-block-start={`${gap}px`}
      {@attach measure}
    >
      <h2 id="surah-page-{entry.localPage}-title" class="sr-only">{copy.range.item("page", entry.pageData.page.globalPage)}</h2>
      {#if entry.pageData.page.startAyah === 1 && headerText(entry.pageData.normalization)}
        <div class="surah-opener-bismillah flex justify-center">
          <Bismillah class="w-44 text-quran-foreground" title={headerText(entry.pageData.normalization) ?? "bismillah"} />
        </div>
      {/if}
      <div class="reading-flow" dir={flowRtl ? "rtl" : undefined}>
        {#each entry.pageData.ayahs as ayah (ayah.key)}
          {@const view = rowView(ayah, entry.pageData)}
          <ReadingAyah text={view.text} n={ayah.ayah} vKey={ayah.key} isTranslation={view.isTranslation ?? showsTranslation} translationLang={view.translationLang ?? routeEntry?.languageCode} pending={view.pending} script={view.script} />
        {/each}
      </div>
    </section>
  {/if}
{/snippet}

<div class="reader-stack flex flex-col gap-4">
  {#if clientMounted && initial.ayahs.length === 0 && !degradation.loadFailed && quran.status !== "error"}
    <span class="sr-only" role="status" aria-live="polite">{copy.shell.opening}</span>
  {/if}


  <!-- No card (user pick): the reader sits on the page, shares the nav/bar left edge, and only
       the rule under the surah header and the rules between ayahs divide it. -->
  <div>
    <ReaderHeader
      {initial}
      {clientMounted}
      readingText={flowId === null ? "arabic" : "translation"}
      readingFlowName={flowName}
      onReadArabic={readArabic}
      onReadTranslation={readTranslation}
      onPickTranslation={() => (readingPickerOpen = true)}
    />

    <TranslationModal bind:open={readingPickerOpen} {readPick} />

    <div class="sr-only" aria-live="polite">{stackedAnnouncement}</div>
    <div
      {@attach captureReaderPages}
      class="reader-pages"
      data-source-kind={showsTranslation ? "translation" : "arabic"}
      tabindex="-1"
    >
      <TooltipProvider delayDuration={300}>
        {#if reader.isVerseMode && firstLoaded.page.startAyah === 1 && headerText(firstLoaded.normalization)}
          <div class="surah-opener-bismillah flex justify-center">
            <Bismillah class="w-44 text-quran-foreground" title={headerText(firstLoaded.normalization) ?? "bismillah"} />
          </div>
        {/if}
        <ReaderVirtualList
          bind:this={virtualList}
          items={renderItems}
          tag={reader.isVerseMode ? "ol" : "div"}
          layoutKey={renderLayoutKey}
          preserving={suppressScroll || anchorScrolling}
          onRendered={syncRendered}
          onResize={repairReaderItem}
          item={renderReaderItem}
        />
      </TooltipProvider>
    </div>

    <span class="sr-only" aria-live="polite">
      {positionLabel(copy, reader.position ?? { globalPage: visiblePageData?.page.globalPage ?? 1 })}
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
      currentSurah={initial.surah}
      ctx={routeContext}
      {previousSurah}
      {nextSurah}
      degraded={degradation.loadFailed}
      {previousPage}
      {nextPage}
    />
  </div>
</div>

<style>
  .reader-pages {
    overflow-anchor: none;
    outline: none;
  }

  /* §22 Bismillah/opener: the calligraphy SVG replaces the text opener (Surah 1
     never renders one — its bismillah is ayah 1). Padding, not margin: in verse
     mode the section carries no padding, so a margin would collapse through the
     section edge, while reading mode adds 2rem section padding — the two modes
     landed the calligraphy at different heights. Padding-block pins it to the
     same 44px from the card top; the reading-mode rule sheds the 2rem the
     section padding already contributes. */
  .surah-opener-bismillah {
    padding-block: 44px;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-page {
    border-bottom: 1px solid var(--reader-divider);
    padding: 2rem 0;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-opener-bismillah {
    padding-block: 12px;
  }

  :global([data-reader-mode="reading"]) .reader-pages .surah-page[data-last-page] {
    border-bottom: 0;
  }

  :global([data-reader-mode="reading"]) .reader-pages[data-source-kind="arabic"] .reading-flow {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
    font-family: var(--reader-arabic-family, var(--font-arabic));
    line-height: 2.35;
    word-spacing: 0.14em;
  }

  /* U10: translations flow as one continuous justified column in reading
     mode (per-ayah rows remain in verse mode). Direction stays per-verse via
     dir="auto" on the verse text. */
  :global([data-reader-mode="reading"]) .reader-pages[data-source-kind="translation"] .reading-flow {
    display: block;
    text-align: justify;
    font-family: var(--reader-translation-family, var(--font-sans));
    line-height: 1.9;
  }

</style>
