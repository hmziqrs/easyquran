<script lang="ts">
  import { onMount } from "svelte";
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
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import type { Ayah, RangePageData, SurahNormalization } from "$lib/data/quran-types";
  import { bodyText } from "$lib/quran/view/source-view";
  import { groupRangeAyahs } from "$lib/quran/view/presentation";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import {
    createRangeReaderCoordinator,
    quranWorker,
    rangeRouteKey,
    type RangeDisplaySnapshot,
    type RangeRouteKey,
  } from "$lib/quran/worker-client";
  import { ReadChainError } from "$lib/quran/fetch";

  let { data }: { data: RangePageData } = $props();
  const copy = getReaderUiCopy();

  const coord = createRangeReaderCoordinator();
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
  const viewKey = $derived(`${rangeSourceId}:${data.kind}:${data.index}`);
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
      const validate = (globalIndex: number, surah: number, ayah: number): boolean =>
        quranData.globalIndexOf(surah, ayah) === globalIndex;
      const range = await quranWorker.readRange(
        serverData.startGlobal,
        serverData.endGlobal,
        validate,
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
    const sourceId = rangeSourceId;
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
  {#each groups as g (g.surah.num)}
    <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
      <div class="flex items-center justify-between gap-3 border-b border-line px-5 py-3 sm:px-9">
        <span class="text-sm font-semibold text-fg">{g.surah.num}. {g.surah.name}</span>
        <button
          type="button"
          onclick={() => openSurah(g.surah)}
          aria-label={copy.range.fullSurah}
          title={copy.range.fullSurah}
          class="flex items-center gap-2 text-[12.5px] text-accent transition-colors hover:brightness-110"
        >
          <span dir="rtl" class="font-arabic text-base">{g.surah.arabic}</span>
          <span>{copy.range.fullSurah}</span>
        </button>
      </div>
      {#if g.opener}
        <p dir="rtl" class="border-b border-line px-5 py-4 text-center font-arabic text-fg-3 sm:px-9">
          {g.opener}
        </p>
      {/if}
      <TooltipProvider delayDuration={300}>
        <ol class="ayah-list flex list-none flex-col p-0">
          {#each g.ayahs as a (a.key)}
            <VerseRow text={bodyText(a.text, a.ayah, g.normalization)} n={a.ayah} vKey={a.key} />
          {/each}
        </ol>
      </TooltipProvider>
    </div>
  {/each}

  {#if displayed.ayahs.length === 0}
    <div
      class="rounded-2xl border border-line bg-bg-1 px-5 py-10 text-center text-sm text-fg-2 sm:px-9"
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
      class="flex items-center justify-between gap-4 rounded-2xl border border-line bg-bg-1 px-5 py-[22px] sm:px-9"
    >
      {#if prevHref}
        <a
          href={publicHref(prevHref)}
          data-sveltekit-preload-data="hover"
          aria-label={copy.range.item(data.kind, data.index - 1)}
          title={copy.range.item(data.kind, data.index - 1)}
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
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
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {copy.range.item(data.kind, data.index + 1)}
          <span aria-hidden="true">→</span>
        </a>
      {/if}
    </div>
  {/if}
</div>

<style>
  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .ayah-list {
    display: block;
    direction: rtl;
    text-align: justify;
    text-align-last: center;
  }
</style>
