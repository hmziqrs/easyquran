<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { reader } from "$lib/stores/reader.svelte";
  import { surahAyahPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import type { QuranData } from "$lib/data/quran-data";
  import { quranSearch } from "$lib/quran/search";
  import {
    SearchHitKind,
    SearchProvider,
    searchHitAnchorAyah,
    searchHitKey,
    searchHitSurah,
    searchHitText,
    type SearchHit,
    type SearchResponse,
  } from "$lib/quran/search/types";
  import SearchResultText from "./SearchResultText.svelte";

  interface SearchState {
    result: SearchResponse;
    quranData: QuranData;
  }

  const routeContext = $derived.by<SurahRouteContext>(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (typeof lang === "string" && typeof translator === "string") {
      return { kind: "translation", lang, translator };
    }
    return { kind: "arabic" };
  });

  const searchPromise = $derived.by((): Promise<SearchState | null> => {
    const query = reader.query.trim();
    if (!query) return Promise.resolve(null);
    return new Promise<void>((resolveDelay) => setTimeout(resolveDelay, 140)).then(async () => {
      if (reader.query.trim() !== query) return null;
      const [result, quranData] = await Promise.all([quranSearch(query), loadQuranData()]);
      return { result, quranData };
    });
  });

  function resultLabel(result: SearchResponse): string {
    if (result.total === 0) return `No verses match “${result.query.trim()}”.`;
    if (result.source === SearchProvider.Names) {
      return `${result.total} surah${result.total === 1 ? "" : "s"} matching “${result.query.trim()}”`;
    }
    return `${result.total} Quran text result${result.total === 1 ? "" : "s"} matching “${result.query.trim()}”`;
  }

  function open(r: SearchHit, quranData: QuranData): void {
    const surah = searchHitSurah(r);
    const ayah = searchHitAnchorAyah(r);
    const entry = quranData.surahByNum(surah);
    if (!entry) return;
    const localPage = quranData.surahLocalPageForAyah(surah, ayah);
    if (!localPage) return;
    reader.openVerse(surah, ayah);
    void goto(resolve(surahAyahPathFor(routeContext, entry, localPage.localPage, ayah)));
  }
</script>

<div class="flex flex-col gap-3">
  {#await searchPromise}
    <div class="text-sm text-fg-2">Searching…</div>
  {:then state}
    {#if state}
      <div class="text-sm text-fg-2">{resultLabel(state.result)}</div>
      {#each state.result.results as r (searchHitKey(r))}
        {@const surah = searchHitSurah(r)}
        {@const text = searchHitText(r)}
        <button
          type="button"
          onclick={() => open(r, state.quranData)}
          class="flex flex-col gap-2.5 rounded-[13px] border border-line bg-bg-1 px-6 py-5 text-left transition-colors hover:border-accent"
        >
          <span class="text-xs font-semibold uppercase tracking-[0.08em] text-accent">
            {#if r.kind === SearchHitKind.Opener}
              {state.quranData.surahByNum(surah)?.name ?? `Surah ${surah}`} · Surah opener
            {:else}
              {state.quranData.surahByNum(surah)?.name ?? `Surah ${surah}`} {surah}:{r.ayah.ayah}
            {/if}
          </span>
          {#if text}
            <SearchResultText {text} highlights={r.highlights} />
          {:else}
            <span class="text-sm text-fg-3">Open surah →</span>
          {/if}
        </button>
      {/each}
    {/if}
  {:catch}
    <div class="text-sm text-fg-2">Search is temporarily unavailable.</div>
  {/await}
</div>
