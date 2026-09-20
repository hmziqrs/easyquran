<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import {
    SourceKind,
    globalPagePathFor,
    juzPathFor,
    routeContextFromParams,
    surahPathFor,
    translationIdFromSegments,
    type SurahLink,
  } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { trackReaderView } from "$lib/quran/track-view.svelte";
  import VerseRow from "./VerseRow.svelte";
  import {
    createStackedTranslations,
    erroredFor,
    loadingFor,
    stackedFor,
  } from "./stacked-translations.svelte";
  import { TRANSLATION_CATALOGUE } from "$lib/quran/catalogue";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  // Direct import: the brand barrel pulls Brand -> config/site -> $env/dynamic,
  // which must not enter this module graph.
  import Bismillah from "$lib/components/brand/Bismillah.svelte";
  import type { Ayah, RangePageData, SurahNormalization } from "$lib/data/quran-types";
  import { bodyText } from "$lib/quran/view/source-view";
  import { groupRangeAyahs } from "$lib/quran/view/presentation";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { reader } from "$lib/stores/reader.svelte";
  import { publicHref } from "$lib/i18n/public-href";
  import {
    createRangeReaderCoordinator,
    quranWorker,
    rangeRouteKey,
    type RangeDisplaySnapshot,
    type RangeRouteKey,
  } from "$lib/quran/worker-client";
  import { ReadChainError } from "$lib/quran/fetch";
  import { ayahIndexValidator } from "./range-validate";

  let { data }: { data: RangePageData } = $props();
  const copy = getReaderUiCopy();

  const coord = createRangeReaderCoordinator();
  let stackedQuranData = $state<Awaited<ReturnType<typeof loadQuranData>> | null>(null);
  const stackedController = createStackedTranslations({
    from: () => data.startGlobal,
    to: () => data.endGlobal,
    validator: () => (stackedQuranData ? ayahIndexValidator(stackedQuranData) : null),
    primarySourceId: () => {
      const lang = page.params.lang;
      const translator = page.params.translator;
      return lang && translator ? translationIdFromSegments(lang, translator) : null;
    },
    catalogue: () => TRANSLATION_CATALOGUE,
    routeKey: () => `${data.kind}:${data.index}`,
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
  let readStatus = $state<"ready" | "loading" | "offline" | "error">("loading");
  // Intentional SSR snapshot; the route-keyed pre-effect installs later prop updates.
  // svelte-ignore state_referenced_locally
  let displayed = $state.raw<RangeDisplaySnapshot>({
    ayahs: data.ayahs,
    normalizations: data.normalizations,
    surahs: data.surahs,
  });

  interface RenderedGroup {
    surah: SurahLink;
    ayahs: readonly Ayah[];
    normalization: SurahNormalization;
    opener: string | null;
  }

  const groups = $derived.by<RenderedGroup[]>(() => {
    const list = groupRangeAyahs(displayed.ayahs, displayed.normalizations);
    const byNum = new Map<number, SurahLink>();
    for (const surah of displayed.surahs) byNum.set(surah.num, surah);
    const out: RenderedGroup[] = [];
    for (const g of list) {
      const surah = byNum.get(g.surah);
      if (surah) {
        out.push({ surah, ayahs: g.ayahs, normalization: g.normalization, opener: g.opener });
      }
    }
    return out;
  });

  const ctx = $derived(routeContextFromParams(page.params));

  const rangeSourceId = $derived(
    ctx.kind === SourceKind.Arabic ? null : translationIdFromSegments(ctx.lang, ctx.translator),
  );
  const isArabic = $derived(rangeSourceId === null);
  // Arabic range reads serve the persisted script preference (variant corpus via
  // the worker/API ladder); the key includes it so a script switch re-reads.
  const readSourceId = $derived(rangeSourceId ?? reader.arabicScript);
  const viewKey = $derived(`${readSourceId}:${data.kind}:${data.index}`);
  trackReaderView({ key: () => viewKey, sourceId: () => rangeSourceId });

  function openSurah(surah: SurahLink): void {
    void goto(publicHref(readerHrefFor(copy.locale, surahPathFor(ctx, surah))));
  }

  const MAX = $derived(data.kind === "juz" ? 30 : 604);
  function rangeHref(kind: RangePageData["kind"], index: number): `/${string}` {
    const quranHref = kind === "juz" ? juzPathFor(ctx, index) : globalPagePathFor(ctx, index);
    return readerHrefFor(copy.locale, quranHref);
  }

  const prevHref = $derived(data.index > 1 ? rangeHref(data.kind, data.index - 1) : null);
  const nextHref = $derived(data.index < MAX ? rangeHref(data.kind, data.index + 1) : null);

  async function runClientRead(
    key: RangeRouteKey,
    serverData: RangePageData,
    sourceId: string | null,
  ): Promise<void> {
    readStatus = "loading";
    try {
      const quranData = await loadQuranData();
      const range = await quranWorker.readRange(
        serverData.startGlobal,
        serverData.endGlobal,
        ayahIndexValidator(quranData),
        sourceId ?? undefined,
      );
      const byNum = new Map<number, SurahLink>();
      for (const s of quranData.surahs) byNum.set(s.num, s);
      const surahs: SurahLink[] = [];
      const seen = new Set<number>();
      for (const a of range.ayahs) {
        if (seen.has(a.surah)) continue;
        seen.add(a.surah);
        const entry = byNum.get(a.surah);
        if (entry) surahs.push({ num: entry.num, slug: entry.slug, name: entry.name, arabic: entry.arabic });
      }
      const snapshot: RangeDisplaySnapshot = {
        ayahs: range.ayahs,
        normalizations: range.normalizations,
        surahs,
      };
      if (coord.applyClientResult(key, snapshot).applied) {
        displayed = coord.currentSnapshot();
        readStatus = "ready";
      }
    } catch (err) {
      const failure =
        err instanceof ReadChainError ? err.workerFailure ?? err.apiFailure : undefined;
      coord.markFailed(key, failure);
      displayed = coord.currentSnapshot();
      readStatus = failure?.kind === "transport" || failure?.kind === "timeout" || failure?.kind === "worker"
        ? "offline"
        : "error";
    }
  }

  $effect.pre(() => {
    const serverData = data;
    const sourceId = readSourceId;
    const key = rangeRouteKey(sourceId, serverData.kind, serverData.index);
    const serverSnapshot: RangeDisplaySnapshot = {
      ayahs: serverData.ayahs,
      normalizations: serverData.normalizations,
      surahs: serverData.surahs,
    };
    const decision = coord.installServer(key, serverSnapshot);
    displayed = serverSnapshot;
    readStatus = serverSnapshot.ayahs.length > 0 ? "ready" : "loading";
    if (decision.read) void runClientRead(key, serverData, sourceId);
  });

  onMount(() => {
    const stop = quranWorker.onStatus((status) => {
      if (status !== "ready") return;
      const retryKey = coord.canRetry();
      if (!retryKey) return;
      void runClientRead(retryKey, data, rangeSourceId);
    });
    return stop;
  });
</script>

<div
  class="flex flex-col gap-4"
  data-source-kind={isArabic ? "arabic" : "translation"}
  aria-busy={readStatus === "loading"}
>
  <div class="sr-only" aria-live="polite">{stackedAnnouncement}</div>
  {#each groups as g (g.surah.num)}
    <div class="overflow-hidden rounded-lg border border-border bg-reader-background">
      <div class="flex items-center justify-between gap-3 border-b border-border px-5 py-3 sm:px-9">
        <span class="text-sm font-semibold text-foreground">{g.surah.num}. {g.surah.name}</span>
        <button
          type="button"
          onclick={() => openSurah(g.surah)}
          aria-label={copy.range.fullSurah}
          title={copy.range.fullSurah}
          class="flex items-center gap-2 text-[12.5px] text-primary transition-colors hover:brightness-110"
        >
          <span dir="rtl" lang="ar" class="font-arabic text-base">{g.surah.arabic}</span>
          <span>{copy.range.fullSurah}</span>
        </button>
      </div>
      {#if g.opener}
        <div
          class="surah-opener-bismillah flex justify-center border-b border-reader-divider px-5 pb-[22px] sm:px-9"
        >
          <Bismillah class="w-44 text-quran-foreground" title={g.opener} />
        </div>
      {/if}
      <TooltipProvider delayDuration={300}>
        <ol class="ayah-list flex list-none flex-col p-0">
          {#each g.ayahs as a (a.key)}
            <VerseRow
              text={bodyText(a.text, a.ayah, g.normalization)}
              n={a.ayah}
              vKey={a.key}
              script={g.normalization.script}
              stacked={stackedFor(stackedController.state, a.key)}
              stackedPending={loadingFor(stackedController.state, a.key)}
              stackedErrored={erroredFor(stackedController.state, a.key)}
              stackedErrorLabel={copy.stacked.error}
            />
          {/each}
        </ol>
      </TooltipProvider>
    </div>
  {/each}

  {#if displayed.ayahs.length === 0}
    <div
      class="rounded-lg border border-border bg-surface px-5 py-10 text-center text-sm text-foreground-secondary sm:px-9"
      role={readStatus === "loading" ? "status" : "alert"}
      aria-live={readStatus === "loading" ? "polite" : "assertive"}
    >
      {#if readStatus === "loading"}
        {copy.sidebar.loadingNavigation}
      {:else if readStatus === "offline"}
        {copy.shell.networkUnavailable}
      {:else}
        {isArabic ? copy.shell.networkUnavailable : copy.range.translationUnavailable}
      {/if}
    </div>
  {/if}

  {#if prevHref || nextHref}
    <div
      class="flex items-center justify-between gap-4 rounded-lg border border-border bg-surface px-5 py-[22px] sm:px-9"
    >
      {#if prevHref}
        <a
          href={publicHref(prevHref)}
          data-sveltekit-preload-data="hover"
          aria-label={copy.range.item(data.kind, data.index - 1)}
          title={copy.range.item(data.kind, data.index - 1)}
          class="flex items-center gap-1.5 text-sm text-foreground-secondary transition-colors hover:text-foreground"
        >
          <span aria-hidden="true">←</span>
          {copy.range.item(data.kind, data.index - 1)}
        </a>
      {:else}
        <span></span>
      {/if}
      {#if nextHref}
        <a
          href={publicHref(nextHref)}
          data-sveltekit-preload-data="hover"
          aria-label={copy.range.item(data.kind, data.index + 1)}
          title={copy.range.item(data.kind, data.index + 1)}
          class="flex items-center gap-1.5 text-sm text-foreground-secondary transition-colors hover:text-foreground"
        >
          {copy.range.item(data.kind, data.index + 1)}
          <span aria-hidden="true">→</span>
        </a>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* §22 Bismillah/opener: calligraphy SVG; margins mirror the Surah reader. */
  .surah-opener-bismillah {
    margin-block: 44px;
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .ayah-list {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
  }

  /* U10: continuous justified flow for translations in reading mode (mirrors
     SurahReader; verse mode keeps per-ayah rows). */
  :global([data-reader-mode="reading"] [data-source-kind="translation"]) .ayah-list {
    display: block;
    text-align: justify;
  }
</style>
