<script lang="ts">
  import { resolve } from "$app/paths";
  import {
    surahLocalPagePathFor,
    surahRouteContext,
    type SurahLocalPageData,
    type SurahLocalPageLink,
    type SurahLink,
  } from "$lib/data/quran";

  let {
    initial,
    lastLoadedLocalPage,
    previousPage,
    nextPage,
    previousSurah,
    nextSurah,
  }: {
    initial: SurahLocalPageData;
    lastLoadedLocalPage: number;
    previousPage: SurahLocalPageLink | null;
    nextPage: SurahLocalPageLink | null;
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
  } = $props();

  const ctx = $derived(surahRouteContext(initial.normalization.sourceId));
  const previousHref = $derived.by(() => {
    if (lastLoadedLocalPage === initial.page.localPage && previousPage) {
      return previousPage.href;
    }
    if (lastLoadedLocalPage > 1) {
      return surahLocalPagePathFor(ctx, initial.surah, lastLoadedLocalPage - 1);
    }
    return previousSurah ? surahLocalPagePathFor(ctx, previousSurah, 1) : null;
  });
  const nextHref = $derived.by(() => {
    if (lastLoadedLocalPage === initial.page.localPage && nextPage) {
      return nextPage.href;
    }
    if (lastLoadedLocalPage < initial.pageCount) {
      return surahLocalPagePathFor(ctx, initial.surah, lastLoadedLocalPage + 1);
    }
    return nextSurah ? surahLocalPagePathFor(ctx, nextSurah, 1) : null;
  });
  const previousLabel = $derived(
    lastLoadedLocalPage > 1 ? `Page ${lastLoadedLocalPage - 1}` : previousSurah?.name,
  );
  const nextLabel = $derived(
    lastLoadedLocalPage < initial.pageCount ? `Page ${lastLoadedLocalPage + 1}` : nextSurah?.name,
  );
</script>

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
