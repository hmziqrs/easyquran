<script lang="ts">
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import {
    globalPagePathFor,
    surahPathFor,
    type MushafPageLink,
    type SurahLink,
    type SurahRouteContext,
  } from "#lib/data/quran.js";

  let {
    currentSurah,
    ctx,
    previousSurah,
    nextSurah,
    degraded = false,
    previousPage = null,
    nextPage = null,
  }: {
    /** The surah being read — read-again target; the reader header already carries the rest. */
    currentSurah: Pick<SurahLink, "num" | "slug" | "name">;
    /** Active translation context, so every generated link keeps the source. */
    ctx: SurahRouteContext;
    /** Cross-surah navigation — always rendered, infinite scroll never crosses it. */
    previousSurah: SurahLink | null;
    nextSurah: SurahLink | null;
    /** When the source can't stream (API unreachable, degraded read), the
     * adjacent-page links come back: manual page jumps are the only way forward. */
    degraded?: boolean;
    previousPage?: MushafPageLink | null;
    nextPage?: MushafPageLink | null;
  } = $props();

  const copy = getReaderUiCopy();

  const startArrow = $derived(copy.direction === "rtl" ? "→" : "←");
  const endArrow = $derived(copy.direction === "rtl" ? "←" : "→");

  function surahHref(surah: Pick<SurahLink, "slug">): `/${string}` {
    return readerHrefFor(copy.locale, surahPathFor(ctx, surah));
  }

  function pageHref(globalPage: number): `/${string}` {
    return readerHrefFor(copy.locale, globalPagePathFor(ctx, globalPage));
  }
</script>

<nav aria-label={copy.shell.surahNavLabel} class="flex flex-col gap-3">
  {#if degraded && (previousPage || nextPage)}
    <div class="flex flex-wrap items-center justify-center gap-2 text-[12.5px] text-muted">
      <span>{copy.shell.manualPagesLabel}</span>
      {#if previousPage}
        <a
          href={publicHref(pageHref(previousPage.globalPage))}
          data-sveltekit-preload-data="hover"
          class="rounded-pill border border-border px-2.5 py-1 font-mono transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {copy.range.item("page", previousPage.globalPage)}
        </a>
      {/if}
      {#if nextPage}
        <a
          href={publicHref(pageHref(nextPage.globalPage))}
          data-sveltekit-preload-data="hover"
          class="rounded-pill border border-border px-2.5 py-1 font-mono transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {copy.range.item("page", nextPage.globalPage)}
        </a>
      {/if}
    </div>
  {/if}

  <!-- End-of-surah band: same surface ground as the footer. The breakout margins cancel the
       Container gutter + 1200px centering, so the band meets both viewport edges (body clips
       the vw overshoot). Content stays on the reader column measure inside. -->
  <div class="mx-[calc(50%_-_50vw)] border-t border-border bg-surface px-6 py-9">
    <div class="mx-auto flex w-full max-w-[42rem] flex-col items-center gap-5 text-center">
      <h2 class="text-[17px] font-semibold text-foreground">
        {copy.shell.endOfSurah(currentSurah.name)}
      </h2>
      <a
        href={publicHref(surahHref(currentSurah))}
        data-sveltekit-preload-data="hover"
        class="rounded-pill border border-border px-4 py-1.5 text-[13px] font-medium text-foreground transition-colors hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        {copy.shell.readAgain}
      </a>
      <div class="grid w-full gap-2 sm:grid-cols-2">
        {#if previousSurah}
          <a
            href={publicHref(surahHref(previousSurah))}
            data-sveltekit-preload-data="hover"
            aria-label="{copy.shell.prevSurahLabel}: {previousSurah.name}"
            title="{copy.shell.prevSurahLabel}: {previousSurah.name}"
            class="flex min-w-0 items-center gap-2 rounded-lg border border-transparent bg-surface-hover px-4 py-3 text-start transition-colors duration-150 ease-out hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            <span aria-hidden="true" class="flex-none">{startArrow}</span>
            <span class="flex min-w-0 flex-col">
              <span class="flex min-w-0 items-center gap-1.5">
                <span dir="ltr" class="flex-none text-[12px] font-semibold text-muted"
                  >{previousSurah.num}</span
                >
                <span class="truncate text-sm font-medium text-foreground"
                  >{previousSurah.name}</span
                >
                <span class="truncate font-arabic text-[15px] text-foreground-secondary"
                  >{previousSurah.arabic}</span
                >
              </span>
              <span class="truncate text-[11.5px] text-muted">{previousSurah.meaning}</span>
            </span>
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
            class="flex min-w-0 items-center justify-end gap-2 rounded-lg bg-primary px-4 py-3 text-end transition-colors duration-150 ease-out hover:bg-primary-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            <span class="flex min-w-0 flex-col">
              <span class="flex min-w-0 items-center justify-end gap-1.5">
                <span dir="ltr" class="flex-none text-[12px] font-semibold text-primary-foreground/70"
                  >{nextSurah.num}</span
                >
                <span class="truncate text-sm font-medium text-primary-foreground"
                  >{nextSurah.name}</span
                >
                <span class="truncate font-arabic text-[15px] text-primary-foreground/85"
                  >{nextSurah.arabic}</span
                >
              </span>
              <span class="truncate text-[11.5px] text-primary-foreground/75"
                >{nextSurah.meaning}</span
              >
            </span>
            <span aria-hidden="true" class="flex-none">{endArrow}</span>
          </a>
        {/if}
      </div>
    </div>
  </div>
</nav>
