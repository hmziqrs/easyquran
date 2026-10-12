<script lang="ts">
  import { Seo } from "#lib/components/index.js";
  import {
    resumeCtxFor,
    snippetPath,
    surahAyahPathFor,
    toArabicDigits,
  } from "#lib/data/quran.js";
  import { SNIPPET_KIND_LABELS, type SnippetText } from "#lib/data/snippets.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import type { SnippetRefGroup } from "./+page.server";

  let { data } = $props();

  const copy = getReaderUiCopy();
  // Prerendered HTML is Arabic-only; after hydration the "open in reader" link
  // honors the reader's active translation, like every index page.
  const ctx = $derived(resumeCtxFor(reader.lastRead, { kind: "arabic" }));

  function localized(text: SnippetText): string {
    if (copy.locale === "ar") return text.ar;
    return text.en;
  }

  function kindLabel(): string {
    return localized(SNIPPET_KIND_LABELS[data.kind]);
  }

  function readerHref(group: SnippetRefGroup): string {
    const surah = { slug: group.slug, num: group.surah };
    return publicHref(readerHrefFor(copy.locale, surahAyahPathFor(ctx, surah, group.ayahStart)));
  }

  function extentLabel(group: SnippetRefGroup): string {
    if (group.ayahStart === group.ayahEnd) return `${group.surah}:${group.ayahStart}`;
    return `${group.surah}:${group.ayahStart}-${group.ayahEnd}`;
  }

  const title = $derived(localized(data.title));
  const description = $derived(localized(data.description));
  const canonicalPath = $derived(snippetPath(data.slug));
</script>

<Seo path={canonicalPath} title={title} description={description} includeTextVariants={false} />

<ReaderShell textTools={false}>
  {#snippet header()}
    <h1 class="sr-only">{title}</h1>
    <span class="ms-auto rounded-pill border border-border-strong px-2 py-0.5 text-[11.5px] text-muted"
      >{kindLabel()}</span
    >
  {/snippet}

  <div class="mx-auto flex max-w-3xl flex-col gap-8 py-8">
    <header class="flex flex-col gap-2">
      <h2 class="text-xl font-semibold text-foreground">{title}</h2>
      <p class="text-[14px] leading-relaxed text-foreground-secondary">{description}</p>
    </header>

    {#each data.refs as group (group.surah)}
      <section class="flex flex-col gap-4" aria-labelledby="snippet-ref-{group.surah}">
        <div class="flex flex-wrap items-baseline justify-between gap-2 border-b border-reader-divider pb-2">
          <h3 id="snippet-ref-{group.surah}" class="text-[15px] font-medium text-foreground">
            {group.name}
            <span lang="ar" dir="rtl" class="ms-2 text-foreground-secondary">{group.arabicName}</span>
          </h3>
          <a
            href={readerHref(group)}
            class="text-[12.5px] text-primary underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            {extentLabel(group)} →
          </a>
        </div>

        {#each group.ayahs as row (row.key)}
          <p
            dir="rtl"
            lang="ar"
            class="font-arabic text-[1.6rem] leading-[2.15] text-quran-foreground"
          >
            {row.text}<span
              class="ms-2 align-middle font-sans text-[0.62em] text-foreground-secondary"
              >﴿{toArabicDigits(row.ayah)}﴾</span
            >
          </p>
        {/each}
      </section>
    {/each}
  </div>
</ReaderShell>
