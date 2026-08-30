<script lang="ts">
  import { Seo } from "$lib/components";
  import { surahMeta, surahPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { readerCanonicalEntryPath } from "$lib/i18n/seo";
  import { publicHref } from "$lib/i18n/public-href";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { HUE_LEGIBLE, HUE_SOFT, hueSlotFor } from "../_reader/hue-slot";

  let { data } = $props();

  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.surahIndexTitle);
  const seoDescription = $derived(copy.seo.surahIndexDescription);

  function surahHref(slug: string): `/${string}` {
    return readerHrefFor(copy.locale, surahPathFor(arabicCtx, slug));
  }
</script>

<Seo
  path={readerCanonicalEntryPath("surah-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.index.surahsTitle}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted">{data.surahs.length}</span>
  {/snippet}

  <ul class="grid grid-cols-1 gap-2 sm:grid-cols-2">
    {#each data.surahs as surah (surah.num)}
      {@const hue = hueSlotFor(surah.num)}
      <li>
        <a
          href={publicHref(surahHref(surah.slug))}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-3 rounded-lg border border-border px-4 py-3 transition-colors hover:bg-surface-hover"
        >
          <span
            class="flex h-9 min-w-11 flex-none items-center justify-center rounded-pill px-2.5 text-[15px] font-extrabold tabular-nums"
            style:background={HUE_SOFT[hue]}
            style:color={HUE_LEGIBLE[hue]}
          >
            {surah.num}
          </span>
          <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="truncate text-sm font-medium text-foreground">
              {surah.name}
              {#if surah.transliteration}
                <span class="font-normal text-foreground-secondary">· {surah.transliteration}</span>
              {/if}
            </span>
            <span class="truncate text-[11.5px] text-muted">
              {surah.meaning} · {surahMeta(surah)}
            </span>
          </span>
          <span dir="rtl" lang="ar" class="flex-none font-arabic text-[17px] leading-none">
            {surah.arabic}
          </span>
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
