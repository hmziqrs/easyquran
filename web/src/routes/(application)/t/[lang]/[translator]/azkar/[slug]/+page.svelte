<script lang="ts">
  import { page } from "$app/state";
  import { Seo } from "#lib/components/index.js";
  import {
    snippetPathFor,
    surahAyahPathFor,
    toArabicDigits,
    translationIdFromSegments,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { SNIPPET_KIND_LABELS, type SnippetText } from "#lib/data/snippets.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import ReaderShell from "../../../../../_reader/ReaderShell.svelte";
  import { revealVerseMarker, trackReaderThread } from "#lib/sessions/reader-thread.svelte.js";
  import type { TranslatedSnippetRefGroup } from "./+page.server";

  let { data } = $props();

  const copy = getReaderUiCopy();
  const ctx = $derived.by<SurahRouteContext>(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (lang !== undefined && translator !== undefined) {
      return { kind: "translation", lang, translator };
    }
    return { kind: "arabic" };
  });

  function localized(text: SnippetText): string {
    if (copy.locale === "ar") return text.ar;
    return text.en;
  }

  function kindLabel(): string {
    return localized(SNIPPET_KIND_LABELS[data.snippet.kind]);
  }

  function readerHref(group: TranslatedSnippetRefGroup): string {
    const surah = { slug: group.slug, num: group.surah };
    return publicHref(readerHrefFor(copy.locale, surahAyahPathFor(ctx, surah, group.ayahStart)));
  }

  function extentLabel(group: TranslatedSnippetRefGroup): string {
    if (group.ayahStart === group.ayahEnd) return `${group.surah}:${group.ayahStart}`;
    return `${group.surah}:${group.ayahStart}-${group.ayahEnd}`;
  }

  const title = $derived(localized(data.snippet.title));
  const description = $derived(localized(data.snippet.description));
  const canonicalPath = $derived(snippetPathFor(ctx, data.snippet.slug));
  const canonicalPublicPath = $derived(readerHrefFor("en", canonicalPath));
  const currentPublicPath = $derived(readerHrefFor(copy.locale, canonicalPath));

  // A snippet is a bounded reading thread: reading its last verse completes it.
  trackReaderThread({
    key: () => `${data.snippet.slug}:${page.params.lang}:${page.params.translator}`,
    target: () => {
      const first = data.groups[0];
      const last = data.groups.at(-1);
      if (first === undefined || last === undefined) return null;
      const lang = page.params.lang;
      const translator = page.params.translator;
      return {
        scope: `snippet:${data.snippet.slug}`,
        surah: first.surah,
        ayah: first.ayahStart,
        exact: false,
        boundEnd: `${last.surah}:${last.ayahEnd}`,
        sourceId: lang && translator ? translationIdFromSegments(lang, translator) : null,
      };
    },
    onResume: (thread) => void revealVerseMarker(thread.surah, thread.ayah),
  });
</script>

<Seo
  path={canonicalPublicPath}
  title="{title} — {data.translationName}"
  description={description}
  includeTextVariants={false}
  inLanguage={data.languageCode}
  noindex={data.pending}
  crumbs={[
    { name: copy.seo.home, href: "/" },
    { name: title, href: currentPublicPath },
  ]}
/>

<ReaderShell textTools={false}>
  {#snippet header()}
    <h1 class="sr-only">{title} — {data.translationName}</h1>
    <span class="ms-auto rounded-pill border border-border-strong px-2 py-0.5 text-[11.5px] text-muted"
      >{kindLabel()}</span
    >
  {/snippet}

  <div class="mx-auto flex max-w-3xl flex-col gap-8 py-8">
    <header class="flex flex-col gap-2">
      <h2 class="text-xl font-semibold text-foreground">{title}</h2>
      <p class="text-[14px] leading-relaxed text-foreground-secondary">{description}</p>
      <p class="text-[12.5px] text-muted">{data.translationName}</p>
    </header>

    {#each data.groups as group (group.surah)}
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

        {#each group.rows as row (row.key)}
          <div class="flex flex-col gap-1.5" data-verse-anchor={row.key}>
            <p dir="rtl" lang="ar" class="font-arabic text-[1.6rem] leading-[2.15] text-quran-foreground">
              {row.arabic}<span
                class="ms-2 align-middle font-sans text-[0.62em] text-foreground-secondary"
                >﴿{toArabicDigits(row.ayah)}﴾</span
              >
            </p>
            {#if row.translation !== null}
              <p dir={data.direction} lang={data.languageCode} class="text-[15px] leading-relaxed text-foreground-secondary">
                {row.translation}
              </p>
            {/if}
          </div>
        {/each}
      </section>
    {/each}
  </div>
</ReaderShell>
