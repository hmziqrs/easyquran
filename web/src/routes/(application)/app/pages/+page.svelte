<script lang="ts">
  import { Seo } from "$lib/components";
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

  /** Tile tooltip / aria-label: page ref, surah coverage, verse range, sajdas. */
  function pageMeta(page: PageIndexRow): string {
    const names = page.surahs.map((surah) => surah.name).join(" · ");
    let out = `${copy.range.item("page", page.index)} · ${names} · ${page.first} – ${page.last}`;
    if (page.sajdas.length > 0) {
      const refs = page.sajdas.map((sajda) => `${sajda.surah}:${sajda.ayah}`).join(", ");
      out += ` · ${copy.index.sajda} ${refs}`;
    }
    return out;
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

  <p class="mb-3 flex items-center gap-2 text-[11.5px] text-muted">
    <span class="inline-block size-1.5 rounded-full bg-primary" aria-hidden="true"></span>
    {copy.index.pageSajdaLegend}
  </p>

  <ul class="grid grid-cols-4 gap-1.5 min-[420px]:grid-cols-6 sm:grid-cols-8 lg:grid-cols-11">
    {#each data.rows as page (page.index)}
      {@const hue = hueSlotFor(page.index)}
      <li>
        <a
          href={publicHref(pageHref(page.index))}
          data-sveltekit-preload-data="hover"
          title={pageMeta(page)}
          aria-label={pageMeta(page)}
          class="relative flex h-12 items-center justify-center rounded-md transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          style:background={HUE_SOFT[hue]}
          style:color={HUE_LEGIBLE[hue]}
        >
          {#if page.surahStarts.length > 0}
            <span
              class="absolute start-1 top-0.5 text-[9.5px] font-extrabold tabular-nums opacity-90"
              aria-hidden="true"
            >
              {page.surahStarts[0]}{#if page.surahStarts.length > 1}+{/if}
            </span>
          {/if}
          <span class="text-[14px] font-extrabold tabular-nums">{page.index}</span>
          {#if page.sajdas.length > 0}
            <span
              class="absolute bottom-1 end-1 size-1.5 rounded-full bg-primary"
              aria-hidden="true"
            ></span>
          {/if}
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
