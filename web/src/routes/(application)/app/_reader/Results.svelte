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

  let searchPromise = $state<Promise<SearchState | null> | null>(null);

  $effect(() => {
    const query = reader.query.trim();
    if (!query) {
      searchPromise = null;
      return;
    }
    let timer: ReturnType<typeof setTimeout> | null = null;
    let cancelled = false;
    searchPromise = new Promise<SearchState | null>((resolve) => {
      timer = setTimeout(async () => {
        try {
          const [result, quranData] = await Promise.all([quranSearch(query), loadQuranData()]);
          if (cancelled) {
            resolve(null);
            return;
          }
          resolve({ result, quranData });
        } catch {
          resolve(null);
        }
      }, 140);
    });
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
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
