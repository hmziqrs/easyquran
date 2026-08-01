<!--
  RangeReader — renders the ayahs of a juz or page, grouped by surah. Receives
  the ayahs prerendered in page.data (SSG reads the range from quran-uthmani.sqlite
  at build), so there is no loading state. Raw text stays in page data; rendering
  projects ayah bodies and source-exact opener headers through the canonical view.
-->
<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { surahByNum, surahPath } from "$lib/data/quran";
  import VerseRow from "./VerseRow.svelte";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import type { RangePageData } from "$lib/data/quran-types";
  import { bodyText } from "$lib/quran/view/source-view";
  import { groupRangeAyahs } from "$lib/quran/view/presentation";

  let { data }: { data: RangePageData } = $props();

  // Shared with any future SPA range loader; the component only renders it.
  const groups = $derived(groupRangeAyahs(data.ayahs, data.normalizations));

  /** Open the full surah (leaves the range view). */
  function openSurah(num: number): void {
    void goto(resolve(surahPath(num)));
  }

  // Bounded prev/next pagination within the range's family (juz 1..30, page 1..604).
  const MAX = $derived(data.kind === "juz" ? 30 : 604);
  const kindLabel = $derived(data.kind === "juz" ? "Juz" : "Page");
  function rangePath(
    kind: RangePageData["kind"],
    index: number,
  ): `/app/${RangePageData["kind"]}/${number}` {
    return `/app/${kind}/${index}`;
  }

  const prevHref = $derived(data.index > 1 ? rangePath(data.kind, data.index - 1) : null);
  const nextHref = $derived(data.index < MAX ? rangePath(data.kind, data.index + 1) : null);
</script>

<div class="flex flex-col gap-4">
  {#each groups as g (g.surah)}
    {@const surah = surahByNum(g.surah)}
    <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
      <!-- surah group header -->
      <div class="flex items-center justify-between gap-3 border-b border-line px-5 py-3 sm:px-9">
        <span class="text-sm font-semibold text-fg">{g.surah}. {surah.name}</span>
        <button
          type="button"
          onclick={() => openSurah(g.surah)}
          class="flex items-center gap-2 text-[12.5px] text-accent transition-colors hover:brightness-110"
        >
          <span dir="rtl" class="font-arabic text-base">{surah.arabic}</span>
          <span>Full surah →</span>
        </button>
      </div>
      {#if g.opener}
        <p dir="rtl" class="border-b border-line px-5 py-4 text-center font-arabic text-fg-3 sm:px-9">
          {g.opener}
        </p>
      {/if}
      <!-- ayahs -->
      <TooltipProvider delayDuration={300}>
        <div class="flex flex-col">
          {#each g.ayahs as a (a.key)}
            <VerseRow text={bodyText(a.text, a.ayah, g.normalization)} n={a.ayah} vKey={a.key} />
          {/each}
        </div>
      </TooltipProvider>
    </div>
  {/each}

  <!-- prev / next within the family (juz 1..30, page 1..604) -->
  {#if prevHref || nextHref}
    <div
      class="flex items-center justify-between gap-4 rounded-2xl border border-line bg-bg-1 px-5 py-[22px] sm:px-9"
    >
      {#if prevHref}
        <a
          href={resolve(prevHref)}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          <span aria-hidden="true">←</span> {kindLabel} {data.index - 1}
        </a>
      {:else}
        <span></span>
      {/if}
      {#if nextHref}
        <a
          href={resolve(nextHref)}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {kindLabel} {data.index + 1} <span aria-hidden="true">→</span>
        </a>
      {/if}
    </div>
  {/if}
</div>
