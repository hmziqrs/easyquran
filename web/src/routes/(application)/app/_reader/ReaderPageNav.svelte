<script lang="ts">
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import {
    surahLocalPagePathFor,
    type SurahLink,
    type SurahRouteContext,
  } from "$lib/data/quran";

  let {
    currentSurah,
    ctx,
    previousSurah,
    nextSurah,
    degraded = false,
    previousPage = null,
    nextPage = null,
  }: {
    /** The surah being read — anchor for the degraded manual page links. */
    currentSurah: SurahLink;
    /** Active translation context, so degraded page jumps keep the source. */
    ctx: SurahRouteContext;
    /** Cross-surah navigation — always rendered, infinite scroll never crosses it. */
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
    /** When the source can't stream (API unreachable, degraded read), the
     * adjacent-page links come back: manual page jumps are the only way forward. */
    degraded?: boolean;
    previousPage?: { localPage: number } | null;
    nextPage?: { localPage: number } | null;
  } = $props();

  const copy = getReaderUiCopy();

  const startArrow = $derived(copy.direction === "rtl" ? "→" : "←");
  const endArrow = $derived(copy.direction === "rtl" ? "←" : "→");

  function surahHref(surah: SurahLink): `/${string}` {
    return readerHrefFor(copy.locale, surahLocalPagePathFor(ctx, surah, 1));
  }

  function pageHref(localPage: number): `/${string}` {
    return readerHrefFor(copy.locale, surahLocalPagePathFor(ctx, currentSurah, localPage));
  }
</script>

<nav
  aria-label={copy.shell.surahNavLabel}
  class="flex flex-col gap-3 border-t border-border px-5 py-[22px] sm:px-9"
>
  {#if degraded && (previousPage || nextPage)}
    <div class="flex flex-wrap items-center justify-center gap-2 text-[12.5px] text-muted">
      <span>{copy.shell.manualPagesLabel}</span>
      {#if previousPage}
        <a
          href={publicHref(pageHref(previousPage.localPage))}
          data-sveltekit-preload-data="hover"
          class="rounded-pill border border-border px-2.5 py-1 font-mono transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {copy.range.item("page", previousPage.localPage)}
        </a>
      {/if}
      {#if nextPage}
        <a
          href={publicHref(pageHref(nextPage.localPage))}
          data-sveltekit-preload-data="hover"
          class="rounded-pill border border-border px-2.5 py-1 font-mono transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {copy.range.item("page", nextPage.localPage)}
        </a>
      {/if}
    </div>
  {/if}

  <div class="flex items-center justify-between gap-4">
    {#if previousSurah}
      <a
        href={publicHref(surahHref(previousSurah))}
        data-sveltekit-preload-data="hover"
        aria-label="{copy.shell.prevSurahLabel}: {previousSurah.name}"
        title="{copy.shell.prevSurahLabel}: {previousSurah.name}"
        class="flex h-9 items-center gap-1.5 rounded-pill border border-transparent bg-surface-hover px-4 text-sm font-medium text-foreground transition-colors duration-150 ease-out hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        <span aria-hidden="true">{startArrow}</span>
        {previousSurah.name}
      </a>
    {:else}
      <span></span>
    {/if}
    {#if nextSurah}
      <a
        href={publicHref(surahHref(nextSurah))}
        data-sveltekit-preload-data="hover"
        aria-label="{copy.shell.nextSurahLabel}: {nextSurah.name}"
        title="{copy.shell.nextSurahLabel}: {nextSurah.name}"
        class="flex h-9 items-center gap-1.5 rounded-pill bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors duration-150 ease-out hover:bg-primary-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        {nextSurah.name}
        <span aria-hidden="true">{endArrow}</span>
      </a>
    {/if}
  </div>
</nav>
