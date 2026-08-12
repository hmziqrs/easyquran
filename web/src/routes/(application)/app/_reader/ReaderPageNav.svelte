<script lang="ts">
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
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

  const copy = getReaderUiCopy();

  const ctx = $derived(surahRouteContext(initial.normalization.sourceId));
  const previousHref = $derived.by(() => {
    if (lastLoadedLocalPage === initial.page.localPage && previousPage) {
      return readerHrefFor(
        copy.locale,
        surahLocalPagePathFor(ctx, initial.surah, previousPage.localPage),
      );
    }
    if (lastLoadedLocalPage > 1) {
      return readerHrefFor(
        copy.locale,
        surahLocalPagePathFor(ctx, initial.surah, lastLoadedLocalPage - 1),
      );
    }
    return previousSurah
      ? readerHrefFor(copy.locale, surahLocalPagePathFor(ctx, previousSurah, 1))
      : null;
  });
  const nextHref = $derived.by(() => {
    if (lastLoadedLocalPage === initial.page.localPage && nextPage) {
      return readerHrefFor(
        copy.locale,
        surahLocalPagePathFor(ctx, initial.surah, nextPage.localPage),
      );
    }
    if (lastLoadedLocalPage < initial.pageCount) {
      return readerHrefFor(
        copy.locale,
        surahLocalPagePathFor(ctx, initial.surah, lastLoadedLocalPage + 1),
      );
    }
    return nextSurah
      ? readerHrefFor(copy.locale, surahLocalPagePathFor(ctx, nextSurah, 1))
      : null;
  });
  const previousLabel = $derived(
    lastLoadedLocalPage > 1
      ? copy.range.item("page", lastLoadedLocalPage - 1)
      : previousSurah?.name,
  );
  const nextLabel = $derived(
    lastLoadedLocalPage < initial.pageCount
      ? copy.range.item("page", lastLoadedLocalPage + 1)
      : nextSurah?.name,
  );
</script>

<nav
  aria-label={copy.shell.surahPagesLabel}
  class="flex items-center justify-between gap-4 border-t border-line px-5 py-[22px] sm:px-9"
>
  {#if previousHref && previousLabel}
    <a
      href={publicHref(previousHref)}
      data-sveltekit-preload-data="hover"
      aria-label={previousLabel}
      title={previousLabel}
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
      href={publicHref(nextHref)}
      data-sveltekit-preload-data="hover"
      aria-label={nextLabel}
      title={nextLabel}
      class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
    >
      {nextLabel}
      <span aria-hidden="true">→</span>
    </a>
  {/if}
</nav>
