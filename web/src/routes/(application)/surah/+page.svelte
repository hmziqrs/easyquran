<script lang="ts">
  import { Seo } from "#lib/components/index.js";
  import ReaderPrerenderLinks from "#lib/components/i18n/ReaderPrerenderLinks.svelte";
  import { resumeCtxFor, surahMeta, surahPathFor } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { readerCanonicalEntryPath } from "#lib/i18n/seo.js";
  import { publicHref, type PublicHref } from "#lib/i18n/public-href.js";
  import type { PageData } from "./$types";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { HUE_LEGIBLE, HUE_SOFT, hueSlotFor } from "../_reader/hue-slot";

  let { data } = $props();
  // The universal +page.ts load spreads the server load's result through, so
  // data carries the discovery hrefs alongside the surah rows. This guard
  // keeps the invariant loud: if the spread or the server load is ever
  // dropped, prerender fails here instead of shipping a /surah page with no
  // build-time route discovery anchors.
  function prerenderHrefsOf(pageData: PageData & { readerPrerenderHrefs?: PublicHref[] }): PublicHref[] {
    const hrefs = pageData.readerPrerenderHrefs;
    if (hrefs === undefined) {
      throw new Error("[surah] page data is missing readerPrerenderHrefs");
    }
    return hrefs;
  }
  const prerenderHrefs = $derived(prerenderHrefsOf(data));

  // Rows carry the reader's active source after hydration; prerendered HTML stays Arabic.
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.surahIndexTitle);
  const seoDescription = $derived(copy.seo.surahIndexDescription);

  function surahHref(slug: string): `/${string}` {
    return readerHrefFor(copy.locale, surahPathFor(ctx, slug));
  }
</script>

<Seo
  path={readerCanonicalEntryPath("surah-index")}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>
<ReaderPrerenderLinks hrefs={prerenderHrefs} />

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
