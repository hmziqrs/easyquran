<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import {
    surahPath,
    translationGlobalPagePath,
    translationJuzPath,
    translationSurahPath,
  } from "$lib/data/quran";
  import VerseRow from "./VerseRow.svelte";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import type { RangePageData } from "$lib/data/quran-types";
  import { bodyText } from "$lib/quran/view/source-view";
  import { groupRangeAyahs } from "$lib/quran/view/presentation";

  let { data }: { data: RangePageData } = $props();

  const groups = $derived(groupRangeAyahs(data.ayahs, data.normalizations));

  function surahByNum(num: number) {
    return data.surahs.find((surah) => surah.num === num)!;
  }

  const translation = $derived.by<{ lang: string; translator: string } | null>(() => {
    const m = /^\/app\/t\/([^/]+)\/([^/]+)\/(?:page|juz)\//.exec(page.url.pathname);
    return m ? { lang: m[1]!, translator: m[2]! } : null;
  });

  function openSurah(num: number): void {
    const surah = surahByNum(num);
    const target = translation
      ? translationSurahPath(surah.slug, translation.lang, translation.translator)
      : surahPath(surah);
    void goto(resolve(target));
  }

  const MAX = $derived(data.kind === "juz" ? 30 : 604);
  const kindLabel = $derived(data.kind === "juz" ? "Juz" : "Page");
  function rangeHref(kind: RangePageData["kind"], index: number): `/app/${string}` {
    if (translation) {
      return kind === "juz"
        ? translationJuzPath(translation.lang, translation.translator, index)
        : translationGlobalPagePath(translation.lang, translation.translator, index);
    }
    return `/app/${kind}/${index}`;
  }

  const prevHref = $derived(data.index > 1 ? rangeHref(data.kind, data.index - 1) : null);
  const nextHref = $derived(data.index < MAX ? rangeHref(data.kind, data.index + 1) : null);
</script>

<div class="flex flex-col gap-4">
  {#each groups as g (g.surah)}
    {@const surah = surahByNum(g.surah)}
    <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
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
      <TooltipProvider delayDuration={300}>
        <ol class="flex list-none flex-col p-0">
          {#each g.ayahs as a (a.key)}
            <VerseRow text={bodyText(a.text, a.ayah, g.normalization)} n={a.ayah} vKey={a.key} />
          {/each}
        </ol>
      </TooltipProvider>
    </div>
  {/each}

  {#if data.ayahs.length === 0}
    <div
      class="rounded-2xl border border-line bg-bg-1 px-5 py-10 text-center text-sm text-fg-2 sm:px-9"
    >
      This translation couldn't be loaded right now.
    </div>
  {/if}

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
