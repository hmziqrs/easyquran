<script lang="ts">
  import { Chip, Icon, Seo } from "$lib/components";
  import { globalPagePathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { readerCanonicalEntryPath } from "$lib/i18n/seo";
  import { publicHref } from "$lib/i18n/public-href";
  import type { PageIndexRow } from "./+page";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { HUE_LEGIBLE, HUE_SOFT, hueSlotFor } from "../_reader/hue-slot";

  let { data } = $props();

  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.pagesIndexTitle);
  const seoDescription = $derived(copy.seo.pagesIndexDescription);

  function pageHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, globalPagePathFor(arabicCtx, n));
  }

  function surahNames(page: PageIndexRow): string {
    return page.surahs.map((surah) => surah.name).join(" · ");
  }

  /** Tile tooltip / aria-label: page ref, surah coverage, verse range, sajdas. */
  function pageMeta(page: PageIndexRow): string {
    let out = `${copy.range.item("page", page.index)} · ${surahNames(page)} · ${page.first} – ${page.last}`;
    if (page.sajdas.length > 0) {
      const refs = page.sajdas.map((sajda) => `${sajda.surah}:${sajda.ayah}`).join(", ");
      out += ` · ${copy.index.sajda} ${refs}`;
    }
    return out;
  }

  function sajdaChipLabel(page: PageIndexRow): string {
    if (page.sajdas.length === 1) {
      const sajda = page.sajdas[0]!;
      return `${copy.index.sajda} ${sajda.surah}:${sajda.ayah}`;
    }
    return copy.index.sajdaCount(page.sajdas.length);
  }
</script>

<Seo
  path={readerCanonicalEntryPath("pages-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.index.pagesTitle}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted"
      >{copy.index.pageCount(data.rows.length)}</span
    >
  {/snippet}

  <!-- Page cards: mushaf page number, the surah(s) it draws from, its verse range,
       and a chip when it carries a sajda. The small badge on the numeral marks a
       surah opener (a surah that begins on that page). -->
  <ul class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
    {#each data.rows as page (page.index)}
      {@const hue = hueSlotFor(page.index)}
      <li>
        <a
          href={publicHref(pageHref(page.index))}
          data-sveltekit-preload-data="hover"
          title={pageMeta(page)}
          aria-label={pageMeta(page)}
          class="flex items-center gap-3 rounded-lg border border-border px-3.5 py-2.5 transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          <span
            class="relative flex h-9 min-w-9 flex-none items-center justify-center rounded-pill px-2 text-[15px] font-extrabold tabular-nums"
            style:background={HUE_SOFT[hue]}
            style:color={HUE_LEGIBLE[hue]}
          >
            {#if page.surahStarts.length > 0}
              <span
                class="absolute -start-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-pill px-1 text-[9px] font-extrabold tabular-nums"
                style:background={HUE_LEGIBLE[hue]}
                style:color={HUE_SOFT[hue]}
                aria-hidden="true"
              >
                {page.surahStarts[0]}{#if page.surahStarts.length > 1}+{/if}
              </span>
            {/if}
            {page.index}
          </span>
          <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="truncate text-[12.5px] font-semibold text-foreground">
              {surahNames(page)}
            </span>
            <span class="font-mono text-[10.5px] text-muted">{page.first} – {page.last}</span>
          </span>
          {#if page.sajdas.length > 0}
            <Chip accent class="flex-none font-mono">
              <Icon name="moon" size={11} />
              {sajdaChipLabel(page)}
            </Chip>
          {/if}
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
