<script lang="ts">
  import { onDestroy } from "svelte";
  import { Seo } from "#lib/components/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
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
  import type { ShareVerse } from "#lib/stores/reader-share.svelte.js";
  import { revealVerseMarker, trackReaderThread } from "#lib/sessions/reader-thread.svelte.js";
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

  // Every snippet ref files as its own range row in the default folder.
  const rangeBookmarkable = $derived(data.refs.length > 0);

  // A snippet is a bounded reading thread: reading its last verse completes it.
  trackReaderThread({
    key: () => data.slug,
    target: () => {
      const first = data.refs[0];
      const last = data.refs.at(-1);
      if (first === undefined || last === undefined) return null;
      return {
        scope: `snippet:${data.slug}`,
        surah: first.surah,
        ayah: first.ayahStart,
        exact: false,
        boundEnd: `${last.surah}:${last.ayahEnd}`,
        sourceId: null,
      };
    },
    onResume: (thread) => void revealVerseMarker(thread.surah, thread.ayah),
  });
  const allRefsMarked = $derived(
    data.refs.every((group) =>
      bookmarks.isRangeMarked(group.surah, group.ayahStart, group.surah, group.ayahEnd),
    ),
  );

  function onBookmarkRange(): void {
    if (!rangeBookmarkable) return;
    // All marked → this click removes every range; otherwise fill in the gaps.
    for (const group of data.refs) {
      const marked = bookmarks.isRangeMarked(
        group.surah,
        group.ayahStart,
        group.surah,
        group.ayahEnd,
      );
      if (allRefsMarked || !marked) {
        bookmarks.toggleRange(group.surah, group.ayahStart, group.surah, group.ayahEnd);
      }
    }
  }

  // Share (FLOW-05 range variant): verse texts joined + canonical `?ayah=` URL.
  const FEEDBACK_MS = 4000;
  let shareNote = $state("");
  let shareTimer: ReturnType<typeof setTimeout> | null = null;

  function flashShareNote(message: string): void {
    shareNote = message;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (shareNote = ""), FEEDBACK_MS);
  }

  async function onShareRange(): Promise<void> {
    const verses: ShareVerse[] = [];
    for (const group of data.refs) {
      for (const row of group.ayahs) verses.push({ key: row.key, text: row.text });
    }
    const first = verses[0];
    const last = verses[verses.length - 1];
    if (first === undefined || last === undefined) return;
    const result = await reader.shareRange(first.key, last.key, verses);
    if (result === "copied") {
      flashShareNote(copy.verse.copied);
      return;
    }
    if (result === "link-copied") flashShareNote(copy.verse.shareFailed);
    else if (result === "failed") flashShareNote(copy.verse.shareUnavailable);
  }

  onDestroy(() => {
    if (shareTimer) clearTimeout(shareTimer);
  });

  const actionButton =
    "inline-flex h-8 items-center gap-1.5 rounded-lg border border-border-strong px-2.5 text-[12.5px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
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
      <div class="mt-1 flex flex-wrap items-center gap-2">
        {#if rangeBookmarkable}
          <button
            type="button"
            class={actionButton}
            aria-pressed={allRefsMarked}
            onclick={onBookmarkRange}
          >
            <Icon name={allRefsMarked ? "bookmark-fill" : "bookmark"} size={14} />
            {copy.verse.bookmarkRange}
          </button>
        {/if}
        <button type="button" class={actionButton} onclick={onShareRange}>
          <Icon name="share" size={14} />
          {copy.verse.share}
        </button>
        {#if shareNote}
          <span role="status" class="text-[12px] text-muted">{shareNote}</span>
        {/if}
      </div>
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
            data-verse-anchor={row.key}
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
