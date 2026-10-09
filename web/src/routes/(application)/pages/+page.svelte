<script lang="ts">
  import { groupBy } from "es-toolkit";
  import { Icon, Seo } from "#lib/components/index.js";
  import { globalPagePathFor, resumeCtxFor } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { readerCanonicalEntryPath } from "#lib/i18n/seo.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import type { PageIndexRow } from "./+page";
  import ReaderShell from "../_reader/ReaderShell.svelte";
  import { reader } from "#lib/stores/reader.svelte.js";

  let { data } = $props();

  // Rows carry the reader's active source after hydration; prerendered HTML stays Arabic.
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));
  const copy = getReaderUiCopy();

  const seoTitle = $derived(copy.seo.pagesIndexTitle);
  const seoDescription = $derived(copy.seo.pagesIndexDescription);

  function pageHref(n: number): `/${string}` {
    return readerHrefFor(copy.locale, globalPagePathFor(ctx, n));
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

  /** The surah a card names in Arabic: the one opening on the page, else the first on it. */
  function leadSurah(page: PageIndexRow): PageIndexRow["surahs"][number] | undefined {
    const opening = page.surahStarts[0];
    return page.surahs.find((surah) => surah.num === opening) ?? page.surahs[0];
  }

  // Pages grouped under their juz, in mushaf order.
  const sections = $derived(
    Object.entries(groupBy(data.rows, (row) => row.juz))
      .map(([juz, pages]) => ({ juz: Number(juz), pages }))
      .sort((a, b) => a.juz - b.juz),
  );

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

<ReaderShell textTools={false}>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.index.pagesTitle}</h1>
    <span class="ms-auto text-[12.5px] text-muted"
      >{copy.index.pageCount(data.rows.length)}</span
    >
  {/snippet}

  <!-- Page cards grouped under their juz: page tile, the surah(s) the page draws from, its
       verse range (+ a sajda tag), and the leading surah's Arabic name. -->
  <div class="flex flex-col gap-8">
    {#each sections as section (section.juz)}
      {@const firstPage = section.pages[0]}
      {@const lastPage = section.pages[section.pages.length - 1]}
      <section aria-labelledby={`juz-${section.juz}`} class="flex flex-col gap-3">
        <div class="flex items-baseline justify-between gap-3">
          <h2 id={`juz-${section.juz}`} class="text-[15px] font-medium text-foreground">
            {copy.range.item("juz", section.juz)}
          </h2>
          {#if firstPage && lastPage}
            <span class="text-caption text-muted"
              >{copy.range.item("page", firstPage.index)}–{lastPage.index}</span
            >
          {/if}
        </div>
        <ul class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
          {#each section.pages as page (page.index)}
            {@const opens = page.surahStarts.length > 0}
            {@const lead = leadSurah(page)}
            <li>
              <a
                href={publicHref(pageHref(page.index))}
                data-sveltekit-preload-data="hover"
                title={pageMeta(page)}
                aria-label={pageMeta(page)}
                class="group flex min-h-[72px] items-center gap-3.5 rounded-xl border border-border bg-surface/40 px-4 py-3 transition-colors hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              >
                <!-- Neutral page tile; tinted only on a page where a surah begins, so the
                     openings stand out down the list instead of an arbitrary colour cycle. -->
                <span
                  class={[
                    "flex size-11 flex-none items-center justify-center rounded-lg text-[15px] font-medium",
                    opens ? "bg-primary-soft text-primary" : "bg-background-subtle text-foreground",
                  ]}
                >
                  {page.index}
                </span>
                <span class="flex min-w-0 flex-1 flex-col gap-1">
                  <span class="truncate text-[14px] font-medium leading-tight text-foreground">
                    {surahNames(page)}
                  </span>
                  <span class="flex items-center gap-2 text-[12px] leading-none text-muted">
                    <span>{page.first} – {page.last}</span>
                    {#if page.sajdas.length > 0}
                      <span
                        data-sajda
                        class="inline-flex items-center gap-1 text-foreground-secondary"
                        title={sajdaChipLabel(page)}
                      >
                        <Icon name="moon" size={11} />
                        {copy.index.sajda}
                      </span>
                    {/if}
                  </span>
                </span>
                {#if lead}
                  <span
                    lang="ar"
                    dir="rtl"
                    class="flex-none font-arabic text-[19px] leading-none text-foreground-secondary transition-colors group-hover:text-foreground"
                    >{lead.arabic}</span
                  >
                {/if}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
</ReaderShell>
