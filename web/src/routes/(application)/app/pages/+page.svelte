<script lang="ts">
  import { Seo } from "$lib/components";
  import { globalPagePathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { readerCanonicalEntryPath } from "$lib/i18n/seo";
  import { publicHref } from "$lib/i18n/public-href";
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

  <ul class="grid grid-cols-4 gap-1.5 min-[420px]:grid-cols-6 sm:grid-cols-8 lg:grid-cols-11">
    {#each data.rows as page (page.index)}
      {@const hue = hueSlotFor(page.index)}
      <li>
        <a
          href={publicHref(pageHref(page.index))}
          data-sveltekit-preload-data="hover"
          title="{copy.range.item('page', page.index)} · {page.first}"
          class="flex h-11 items-center justify-center rounded-md text-[14px] font-extrabold tabular-nums transition-colors hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          style:background={HUE_SOFT[hue]}
          style:color={HUE_LEGIBLE[hue]}
        >
          {page.index}
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
