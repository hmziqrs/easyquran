<script lang="ts">
  import { onMount } from "svelte";
  import { loadQuranData, peekQuranData } from "$lib/data/quran-data-client";
  import type { QuranData } from "$lib/data/quran-data";
  import {
    parseKey,
    surahAyahPathFor,
    surahRouteContext,
    surahPathFor,
    type SurahRouteContext,
  } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { bookmarksPageHref, readerHrefFor, readerHomeHrefFor, yoursPageHref } from "$lib/i18n/reader";
  import { appLocale } from "$lib/i18n/app-locale";
  import { publicHref } from "$lib/i18n/public-href";
  import { Seo, Icon } from "$lib/components";
  import { Button } from "$lib/components/ui/button";
  import { reader } from "$lib/stores/reader.svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import { parseVerseKey } from "$lib/bookmarks/schema";
  import { resumeToLastRead, resumeToVerse } from "$lib/reader/resume";

  // Reader nav context for opening saved places: like /app/bookmarks, a personal
  // app route has no translation segments, so it always resolves the Arabic reader.
  const ARABIC_CTX: SurahRouteContext = surahRouteContext("uthmani");

  // This URL is never locale-prefixed; the layout's published locale keeps copy
  // aligned with the nav/footer chrome (same seam /app/bookmarks crosses).
  const copy = getReaderUiCopy(appLocale());

  const indexLinks = $derived([
    { label: copy.index.surahsTitle, href: publicHref(readerHrefFor(copy.locale, "/app/surah")) },
    { label: copy.index.juzTitle, href: publicHref(readerHrefFor(copy.locale, "/app/juz")) },
    {
      label: copy.index.pagesTitle,
      href: publicHref(readerHrefFor(copy.locale, "/app/pages")),
    },
    { label: copy.nav.bookmarks, href: publicHref(bookmarksPageHref()) },
  ]);

  let quranData = $state<QuranData | undefined>(peekQuranData());

  interface BookmarkPreviewRow {
    key: string;
    surah: number;
    ayah: number;
    name: string;
    href: string;
  }

  const bookmarkCount = $derived(
    bookmarks.authed ? bookmarks.bookmarks.length : reader.bookmarkedKeys.length,
  );

  const bookmarkRows = $derived.by<BookmarkPreviewRow[]>(() => {
    const data = quranData;
    if (!data) return [];
    interface Coords {
      key: string;
      surah: number;
      ayah: number;
      at: string;
    }
    const coords: Coords[] = bookmarks.authed
      ? bookmarks.bookmarks.map((bookmark) => ({
          key: `${bookmark.surah}:${bookmark.ayah}`,
          surah: bookmark.surah,
          ayah: bookmark.ayah,
          at: bookmark.updatedAt,
        }))
      : reader.bookmarkedKeys.flatMap((key) => {
          const parsed = parseVerseKey(key);
          if (!parsed) return [];
          return [{ key, surah: parsed.surah, ayah: parsed.ayah, at: "" }];
        });
    const sorted = [...coords].sort((a, b) => b.at.localeCompare(a.at));
    return sorted.slice(0, 5).flatMap((entry) => {
      const surah = data.surahByNum(entry.surah);
      if (!surah) return [];
      const localPage = data.surahLocalPageForAyah(entry.surah, entry.ayah)?.localPage ?? 1;
      return [
        {
          key: entry.key,
          surah: entry.surah,
          ayah: entry.ayah,
          name: surah.name,
          href: surahAyahPathFor(ARABIC_CTX, surah, localPage, entry.ayah),
        },
      ];
    });
  });

  const lastRead = $derived(reader.lastRead);
  const lastReadSurah = $derived(lastRead ? quranData?.surahByNum(lastRead.num) : undefined);
  const recents = $derived(
    lastRead
      ? reader.recentReads.filter((r) => r.num !== lastRead.num || r.sourceId !== lastRead.sourceId)
      : reader.recentReads,
  );

  function surahHref(num: number): string | null {
    const surah = quranData?.surahByNum(num);
    if (!surah) return null;
    return publicHref(readerHrefFor(copy.locale, surahPathFor(ARABIC_CTX, surah)));
  }

  function openRecent(num: number, n: number, sourceId: string | undefined): void {
    void resumeToVerse(num, n, sourceId, ARABIC_CTX);
  }

  function openBookmark(row: BookmarkPreviewRow): void {
    const { num, n } = parseKey(`${row.surah}:${row.ayah}`);
    void resumeToVerse(num, n, undefined, ARABIC_CTX);
  }

  onMount(() => {
    reader.hydrate();
    if (peekQuranData() === undefined) {
      void loadQuranData().then(
        (data) => {
          quranData = data;
        },
        () => undefined,
      );
    }
  });
</script>

<svelte:head>
  <title>{copy.index.yoursTitle} · EasyQuran</title>
</svelte:head>

<Seo path={yoursPageHref()} title={copy.seo.yoursTitle} description={copy.seo.yoursDescription} />

<div class="mx-auto max-w-[860px] px-5 pt-8 sm:px-7">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h1 class="text-[28px] font-semibold leading-tight tracking-[-0.02em] text-foreground">
      {copy.index.yoursTitle}
    </h1>
    <nav aria-label={copy.index.yoursTitle} class="flex flex-wrap items-center gap-1.5">
      {#each indexLinks as link (link.href)}
        <a
          href={link.href}
          data-sveltekit-preload-data="hover"
          class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {link.label}
        </a>
      {/each}
    </nav>
  </div>

  {#if lastRead}
    {@const total = lastReadSurah?.ayahCount ?? 0}
    {@const fraction =
      reader.progressFor(lastRead.num) ?? (total > 0 ? Math.min(1, lastRead.n / total) : 0)}
    <section
      aria-labelledby="yours-continue"
      class="mt-5 overflow-hidden rounded-xl border border-border bg-surface"
    >
      <h2 id="yours-continue" class="eyebrow border-b border-border px-4 py-3 sm:px-5">
        {copy.index.yoursContinue}
      </h2>
      <div class="px-4 py-4 sm:px-5">
        <p class="text-xl font-semibold text-foreground">
          {lastReadSurah?.name ?? `Surah ${lastRead.num}`}
        </p>
        <p class="mt-1 text-sm text-foreground-secondary">
          <span class="font-medium text-foreground">{lastRead.num}:{lastRead.n}</span>
          {#if lastReadSurah?.meaning}
            · {lastReadSurah.meaning}
          {/if}
        </p>
        {#if total > 0}
          <div
            class="mt-4 h-1.5 w-full overflow-hidden rounded-pill bg-surface-hover"
            role="progressbar"
            aria-valuenow={lastRead.n}
            aria-valuemax={total}
          >
            <div
              class="h-full rounded-pill bg-primary"
              style:width="{Math.round(fraction * 100)}%"
            ></div>
          </div>
        {/if}
        <div class="mt-4">
          <Button onclick={() => resumeToLastRead(ARABIC_CTX)} arrow>
            {copy.shell.jump}
          </Button>
        </div>
      </div>
    </section>
  {:else}
    <div class="mt-5 flex items-center gap-3.5 rounded-xl border border-border bg-surface px-4 py-4 sm:px-5">
      <span
        class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
        style:background="var(--hue-2-soft)"
        aria-hidden="true"
      >
        <Icon name="book" size={20} />
      </span>
      <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{copy.index.yoursEmpty}</p>
    </div>
  {/if}

  {#if recents.length > 0}
    <section aria-labelledby="yours-recent" class="mt-6">
      <h2 id="yours-recent" class="eyebrow">{copy.index.yoursRecent}</h2>
      <ul class="mt-2 divide-y divide-border overflow-hidden rounded-xl border border-border bg-surface">
        {#each recents as recent (`${recent.num}:${recent.n}:${recent.sourceId ?? ""}`)}
          {@const href = surahHref(recent.num)}
          <li class="flex items-center gap-3 px-4 py-3">
            <button
              type="button"
              class="min-w-0 flex-1 text-start transition-colors hover:text-foreground-secondary"
              onclick={() => openRecent(recent.num, recent.n, recent.sourceId)}
            >
              <span class="block truncate text-sm font-medium text-foreground">
                {quranData?.surahByNum(recent.num)?.name ?? `Surah ${recent.num}`}
              </span>
              <span class="font-mono text-[12px] text-muted">{recent.num}:{recent.n}</span>
            </button>
            {#if href}
              <a
                href={href}
                data-sveltekit-preload-data="hover"
                class="flex h-7 flex-none items-center rounded-pill border border-border px-2.5 text-[11.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              >
                {copy.range.fullSurah}
              </a>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if bookmarkCount > 0}
    <section aria-labelledby="yours-bookmarks" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-bookmarks" class="eyebrow">{copy.nav.bookmarks}</h2>
        <span class="font-mono text-[11px] text-muted">{bookmarkCount}</span>
      </div>
      {#if bookmarkRows.length > 0}
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-xl border border-border bg-surface">
          {#each bookmarkRows as row (row.key)}
            <li>
              <button
                type="button"
                class="flex w-full items-center gap-3 px-4 py-3 text-start transition-colors hover:bg-surface-hover"
                onclick={() => openBookmark(row)}
              >
                <span class="truncate text-sm font-medium text-foreground">{row.name}</span>
                <span class="font-mono text-[12px] text-muted">{row.surah}:{row.ayah}</span>
                <Icon name="bookmark" size={14} class="ms-auto flex-none text-muted" />
              </button>
            </li>
          {/each}
        </ul>
        <div class="mt-2">
          <a
            href={publicHref(bookmarksPageHref())}
            class="inline-flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >
            {copy.index.yoursViewAll}
            <span aria-hidden="true" class="ms-1">→</span>
          </a>
        </div>
      {/if}
    </section>
  {/if}

  <p class="mt-8 text-[11px] text-muted">
    <a href={publicHref(readerHomeHrefFor(copy.locale))} class="underline-offset-4 hover:underline">
      {copy.seo.home}
    </a>
  </p>
</div>
