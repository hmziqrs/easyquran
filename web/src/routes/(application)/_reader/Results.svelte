<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { routeContextFromParams, surahAyahPathFor } from "#lib/data/quran.js";
  import { loadQuranData } from "#lib/data/quran-data-client.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import { quranSearch } from "#lib/quran/search.js";
  import {
    SearchHitKind,
    SearchProvider,
    searchHitAnchorAyah,
    searchHitKey,
    searchHitSurah,
    searchHitText,
    type SearchHit,
    type SearchResponse,
  } from "#lib/quran/search/types.js";
  import { HighlightedArabic } from "#lib/components/text/index.js";

  interface SearchState {
    result: SearchResponse;
    quranData: QuranData;
  }

  const copy = getReaderUiCopy();
  const routeContext = $derived(routeContextFromParams(page.params));

  let searchPromise = $state<Promise<SearchState | null> | null>(null);
  let isSearching = $state(false);

  $effect(() => {
    const query = reader.query.trim();
    if (!query) {
      searchPromise = null;
      isSearching = false;
      return;
    }
    let timer: ReturnType<typeof setTimeout> | null = null;
    let cancelled = false;
    isSearching = true;
    searchPromise = new Promise<SearchState | null>((resolve, reject) => {
      timer = setTimeout(async () => {
        try {
          const [result, quranData] = await Promise.all([quranSearch(query), loadQuranData()]);
          if (cancelled) {
            resolve(null);
            return;
          }
          isSearching = false;
          resolve({ result, quranData });
        } catch (error) {
          if (!cancelled) {
            isSearching = false;
            reject(error);
          }
        }
      }, 140);
    });
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  });

  function resultLabel(result: SearchResponse): string {
    const query = result.query.trim();
    if (result.total === 0) return copy.search.noVerseMatches(query);
    if (result.source === SearchProvider.Names) {
      return copy.search.surahMatches(result.total, query);
    }
    return copy.search.textMatches(result.total, query);
  }

  function open(r: SearchHit, quranData: QuranData): void {
    const surah = searchHitSurah(r);
    const ayah = searchHitAnchorAyah(r);
    const entry = quranData.surahByNum(surah);
    if (!entry) return;
    if (!quranData.surahLocalPageForAyah(surah, ayah)) return;
    reader.openVerse(surah, ayah);
    void goto(
      publicHref(
        readerHrefFor(
          copy.locale,
          surahAyahPathFor(routeContext, entry, ayah),
        ),
      ),
    );
  }
</script>

<div class="flex flex-col gap-3" aria-busy={isSearching}>
  {#await searchPromise}
    <div class="text-sm text-foreground-secondary" role="status" aria-live="polite">{copy.search.searching}</div>
  {:then state}
    {#if state}
      <div class="text-sm text-foreground-secondary" role="status" aria-live="polite">{resultLabel(state.result)}</div>
      {#each state.result.results as r (searchHitKey(r))}
        {@const surah = searchHitSurah(r)}
        {@const text = searchHitText(r)}
        <button
          type="button"
          onclick={() => open(r, state.quranData)}
          class="flex flex-col gap-2.5 rounded-md border border-border bg-surface px-6 py-5 text-start transition-colors hover:border-primary"
        >
          <span class="text-xs font-medium text-muted-foreground">
            {#if r.kind === SearchHitKind.Opener}
              {copy.search.surahOpener(
                state.quranData.surahByNum(surah)?.name ?? copy.sidebar.mode("surah"),
              )}
            {:else}
              {copy.search.ayah(
                state.quranData.surahByNum(surah)?.name ?? copy.sidebar.mode("surah"),
                surah,
                r.ayah.ayah,
              )}
            {/if}
          </span>
          {#if text}
            <HighlightedArabic {text} highlights={r.highlights} />
          {:else}
            <span class="text-sm text-muted-foreground">{copy.search.openSurah}</span>
          {/if}
        </button>
      {/each}
    {/if}
  {:catch}
    <div class="text-sm text-foreground-secondary" role="alert" aria-live="assertive">{copy.search.unavailable}</div>
  {/await}
</div>
