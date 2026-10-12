<script lang="ts">
  import { onMount } from "svelte";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import {
    surahAyahPathFor,
    surahRouteContext,
    surahPathFor,
    type SurahRouteContext,
  } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";
  import { bookmarksPageHref, readerHrefFor, yoursPageHref } from "#lib/i18n/reader.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { Seo, Icon } from "#lib/components/index.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Skeleton } from "#lib/components/ui/skeleton/index.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import { parseVerseKey, verseKeyOf } from "#lib/bookmarks/schema.js";
  import BookmarkRow from "../bookmarks/_components/BookmarkRow.svelte";
  import { resumeToLastRead, resumeToVerse } from "#lib/reader/resume.js";
  import {
    RECENTS_COUNT_CHOICES,
    homeLayout,
    type HomeWidgetId,
  } from "#lib/home/layout.svelte.js";
  import { fetchDailyAyah, type DailyAyah } from "#lib/home/daily-ayah.js";
  import { LOCAL_NOTES_OWNER, notesSource } from "#lib/home/notes-source.svelte.js";
  import {
    languageLabelFor,
    readingChoice,
    suggestedChoice,
    translationForLanguage,
    type ReadingChoice,
  } from "#lib/home/reading-choice.svelte.js";
  import { readingAnalytics } from "#lib/analytics/reading-analytics.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
  import { consent } from "#lib/stores/consent.svelte.js";
  import { sessions } from "#lib/sessions/store.svelte.js";
  import {
    reader_bookmarks_ayah,
    reader_bookmarks_empty_page,
    reader_home_done,
    reader_home_edit,
    reader_home_hide,
    reader_home_hidden,
    reader_home_move_down,
    reader_home_move_up,
    reader_home_read_in,
    reader_home_read_in_arabic,
    reader_home_read_in_with,
    reader_home_reading_in,
    reader_sessions_label,
    reader_sessions_new_add,
    reader_sessions_new_placeholder,
    reader_sessions_sync_error,
    reader_widget_daily_ayah_open,
    reader_widget_daily_ayah_title,
    reader_widget_daily_ayah_unavailable,
    reader_widget_most_read_empty,
    reader_widget_most_read_off,
    reader_widget_most_read_title,
    reader_widget_notes_empty,
    reader_widget_notes_title,
    reader_widget_recents_count,
    reader_widget_recents_empty,
  } from "#lib/i18n/m/reader.js";

  // Reader nav context for opening saved places: like /app/bookmarks, a personal
  // app route has no translation segments, so it always resolves the Arabic reader.
  const ARABIC_CTX: SurahRouteContext = surahRouteContext("uthmani");

  // Saved-translation reopen (Feature 3.5): still in the catalogue → that translation, else Arabic (EC-38).
  function ctxForSavedSource(sourceId: string | null | undefined): SurahRouteContext {
    if (sourceId !== null && sourceId !== undefined && peekTranslationName(sourceId) !== null) {
      return surahRouteContext(sourceId);
    }
    return ARABIC_CTX;
  }

  // This URL is never locale-prefixed; the layout's published locale keeps copy
  // aligned with the nav/footer chrome (same seam /app/bookmarks crosses).
  const locale = appLocale();
  const copy = getReaderUiCopy(locale);
  const msgOptions = { locale };
  const rowCopy = getBookmarksCopy(locale);

  // Widget chrome copy — paraglide fns take the locale explicitly.
  const h = {
    edit: reader_home_edit(undefined, msgOptions),
    done: reader_home_done(undefined, msgOptions),
    hide: reader_home_hide(undefined, msgOptions),
    moveUp: reader_home_move_up(undefined, msgOptions),
    moveDown: reader_home_move_down(undefined, msgOptions),
    hidden: reader_home_hidden(undefined, msgOptions),
    recentsEmpty: reader_widget_recents_empty(undefined, msgOptions),
    recentsCount: reader_widget_recents_count(undefined, msgOptions),
    bookmarksEmpty: reader_bookmarks_empty_page(undefined, msgOptions),
    sessionsLabel: reader_sessions_label(undefined, msgOptions),
    sessionsNewPlaceholder: reader_sessions_new_placeholder(undefined, msgOptions),
    sessionsNewAdd: reader_sessions_new_add(undefined, msgOptions),
    sessionsSyncError: reader_sessions_sync_error(undefined, msgOptions),
    notesTitle: reader_widget_notes_title(undefined, msgOptions),
    notesEmpty: reader_widget_notes_empty(undefined, msgOptions),
    mostReadTitle: reader_widget_most_read_title(undefined, msgOptions),
    mostReadOff: reader_widget_most_read_off(undefined, msgOptions),
    mostReadEmpty: reader_widget_most_read_empty(undefined, msgOptions),
    dailyAyahTitle: reader_widget_daily_ayah_title(undefined, msgOptions),
    dailyAyahUnavailable: reader_widget_daily_ayah_unavailable(undefined, msgOptions),
    dailyAyahOpen: reader_widget_daily_ayah_open(undefined, msgOptions),
    readIn: reader_home_read_in(undefined, msgOptions),
    readInArabic: reader_home_read_in_arabic(undefined, msgOptions),
    readingInName: (name: string): string => reader_home_reading_in({ name }, msgOptions),
    readInWith: (language: string): string =>
      reader_home_read_in_with({ language }, msgOptions),
  };

  // Session switcher (Feature 4.5): the continue card, recents and progress
  // below all read the live mirror, which tracks the active session.
  const sessionOptions = $derived(sessions.list.filter((session) => session.archivedAt === null));
  const activeSessionId = $derived(sessions.activeId ?? "");
  let newSessionName = $state("");

  function addSession(): void {
    const created = sessions.create(newSessionName);
    if (created !== null) newSessionName = "";
  }

  // The widget column renders only after mount: SSR would paint the default
  // layout and reorder on hydrate (the stored layout is device-local).
  let mounted = $state(false);
  let editing = $state(false);

  const widgets = $derived(homeLayout.entries);

  function widgetTitle(id: HomeWidgetId): string {
    if (id === "continue-reading") return copy.index.yoursContinue;
    if (id === "recents") return copy.index.yoursRecent;
    if (id === "notes") return h.notesTitle;
    if (id === "most-read") return h.mostReadTitle;
    if (id === "daily-ayah") return h.dailyAyahTitle;
    return copy.nav.bookmarks;
  }

  // Notes (BM-03): local-only, per-owner. Owner resolution mirrors the reader
  // persistence layer (defer while auth is unresolved, EC-17) — the widget only
  // ever reads the signed-in owner's key, `local` when signed out.
  const notesOwner = $derived.by<string | null>(() => {
    if (authState.status === "unknown") return null;
    if (authState.status === "authenticated" && authState.user !== null) {
      return String(authState.user.id);
    }
    return LOCAL_NOTES_OWNER;
  });
  $effect(() => {
    const owner = notesOwner;
    if (owner !== null) notesSource.setOwner(owner);
  });
  const notesRows = $derived(notesSource.rows.slice(0, 5));

  // Most-read consumes the opt-in first-party analytics surface (Feature 6):
  // consent off = never fetched, local rollup when signed out, server summary
  // when signed in (fetchSummary already picks).
  interface MostReadState {
    readonly status: "idle" | "ready" | "empty";
    readonly top: readonly { readonly surah: number; readonly reads: number }[];
  }
  let mostRead = $state.raw<MostReadState>({ status: "idle", top: [] });

  async function loadMostRead(): Promise<void> {
    const summary = await readingAnalytics.fetchSummary();
    if (summary === null) {
      mostRead = { status: "empty", top: [] };
      return;
    }
    mostRead =
      summary.topSurahs.length > 0
        ? { status: "ready", top: summary.topSurahs.slice(0, 5) }
        : { status: "empty", top: [] };
  }

  // Daily ayah (D07): deterministic per UTC date server-side; one fetch, cached
  // by the browser until UTC midnight.
  let dailyAyah = $state<DailyAyah | null>(null);
  let dailyAyahLoaded = $state(false);

  // First-run reading choice (HOME-04): Arabic only + the browser-language
  // translation (English as the extra option). Persists on first pick; the
  // line disappears once a choice exists.
  interface ChoiceOption {
    readonly choice: ReadingChoice;
    readonly label: string;
  }
  let choiceOptions = $state.raw<readonly ChoiceOption[]>([]);

  function buildChoiceOptions(): void {
    const options: ChoiceOption[] = [{ choice: { mode: "arabic" }, label: h.readInArabic }];
    const suggested = suggestedChoice();
    const push = (choice: ReadingChoice): void => {
      if (choice.mode !== "translation") return;
      if (options.some((option) => option.choice.mode === "translation" && option.choice.sourceId === choice.sourceId)) {
        return;
      }
      const language = languageLabelFor(choice.sourceId) ?? "";
      options.push({ choice, label: h.readInWith(language) });
    };
    if (suggested.mode === "translation") push(suggested);
    const english = translationForLanguage("en");
    if (english !== null) push({ mode: "translation", sourceId: english });
    choiceOptions = options;
  }

  // The Continue card labels the chosen translation (HOME-04) and seeds its
  // resume hrefs with it; an explicit last-read translation still wins
  // (resumeCtxFor prefers the stored sourceId).
  const chosenTranslationName = $derived.by<string | null>(() => {
    const chosen = readingChoice.chosen;
    if (chosen === null || chosen.mode !== "translation") return null;
    return peekTranslationName(chosen.sourceId);
  });
  const continueCtx = $derived.by<SurahRouteContext>(() => {
    const chosen = readingChoice.chosen;
    if (chosen !== null && chosen.mode === "translation") {
      return surahRouteContext(chosen.sourceId);
    }
    return ARABIC_CTX;
  });

  function widgetIndex(id: HomeWidgetId): number {
    return widgets.findIndex((widget) => widget.id === id);
  }

  const recentsLimit = $derived(homeLayout.entries[widgetIndex("recents")]?.settings.count ?? 5);

  const indexLinks = $derived([
    { label: copy.index.surahsTitle, href: publicHref(readerHrefFor(copy.locale, "/surah")) },
    { label: copy.index.juzTitle, href: publicHref(readerHrefFor(copy.locale, "/juz")) },
    {
      label: copy.index.pagesTitle,
      href: publicHref(readerHrefFor(copy.locale, "/pages")),
    },
    { label: copy.nav.bookmarks, href: publicHref(bookmarksPageHref()) },
  ]);

  let quranData = $state<QuranData | undefined>(peekQuranData());

  interface BookmarkPreviewRow {
    /** Authed bookmark id, or the verse key on an anonymous row. */
    key: string;
    surah: number;
    ayah: number;
    name: string;
    href: string;
    updatedAt: string;
    hasNote: boolean;
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
      sourceId: string | null;
    }
    const coords: Coords[] = bookmarks.authed
      ? bookmarks.bookmarks.map((bookmark) => ({
          key: bookmark.id,
          surah: bookmark.surah,
          ayah: bookmark.ayah,
          at: bookmark.updatedAt,
          sourceId: bookmark.sourceId ?? null,
        }))
      : reader.bookmarkedKeys.flatMap((key) => {
          const parsed = parseVerseKey(key);
          if (!parsed) return [];
          // v5 stamp (EC-46): legacy rows without one sort last via "".
          const stamp = reader.bookmarkSavedAt[key] ?? 0;
          return [
            {
              key,
              surah: parsed.surah,
              ayah: parsed.ayah,
              at: stamp > 0 ? new Date(stamp).toISOString() : "",
              sourceId: null,
            },
          ];
        });
    const sorted = [...coords].sort((a, b) => b.at.localeCompare(a.at));
    return sorted.slice(0, 5).flatMap((entry) => {
      const surah = data.surahByNum(entry.surah);
      if (!surah) return [];
      return [
        {
          key: entry.key,
          surah: entry.surah,
          ayah: entry.ayah,
          name: surah.name,
          href: surahAyahPathFor(ctxForSavedSource(entry.sourceId), surah, entry.ayah),
          updatedAt: entry.at,
          hasNote: reader.getNote(verseKeyOf(entry.surah, entry.ayah)).trim().length > 0,
        },
      ];
    });
  });

  const lastRead = $derived(reader.lastRead);
  const lastReadSurah = $derived(lastRead ? quranData?.surahByNum(lastRead.num) : undefined);
  const allRecents = $derived(
    lastRead
      ? reader.recentReads.filter((r) => r.num !== lastRead.num || r.sourceId !== lastRead.sourceId)
      : reader.recentReads,
  );
  const recents = $derived(allRecents.slice(0, recentsLimit));

  function surahHref(num: number): string | null {
    const surah = quranData?.surahByNum(num);
    if (!surah) return null;
    return publicHref(readerHrefFor(copy.locale, surahPathFor(ARABIC_CTX, surah)));
  }

  function openRecent(num: number, n: number, sourceId: string | undefined): void {
    void resumeToVerse(num, n, sourceId, ARABIC_CTX);
  }

  function removeBookmarkRow(row: BookmarkPreviewRow): void {
    if (bookmarks.authed) bookmarks.remove(row.key);
    else reader.toggleBookmark(verseKeyOf(row.surah, row.ayah));
  }

  function openNote(note: { surah: number; ayah: number }): void {
    void resumeToVerse(note.surah, note.ayah, undefined, ARABIC_CTX);
  }

  /** BM-03 rows show the note's first line; notes are plain strings (no dates). */
  function firstNoteLine(text: string): string {
    return text.split("\n")[0] ?? text;
  }

  onMount(() => {
    mounted = true;
    homeLayout.hydrate();
    readingChoice.hydrate();
    if (readingChoice.chosen === null) buildChoiceOptions();
    reader.hydrate();
    if (consent.readingAnalytics) void loadMostRead();
    void fetchDailyAyah().then((ayah) => {
      dailyAyah = ayah;
      dailyAyahLoaded = true;
    });
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

{#snippet editControls(id: HomeWidgetId)}
  {#if editing}
    <span class="flex flex-none items-center gap-1">
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded-pill border border-border text-[12.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        aria-label="{h.moveUp}: {widgetTitle(id)}"
        disabled={widgetIndex(id) === 0}
        onclick={() => homeLayout.move(id, -1)}>↑</button
      >
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded-pill border border-border text-[12.5px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        aria-label="{h.moveDown}: {widgetTitle(id)}"
        disabled={widgetIndex(id) === widgets.length - 1}
        onclick={() => homeLayout.move(id, 1)}>↓</button
      >
      <button
        type="button"
        class="flex h-7 items-center rounded-pill border border-border px-2.5 text-[12px] text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        onclick={() => homeLayout.toggle(id)}>{h.hide}</button
      >
    </span>
  {/if}
{/snippet}

<div class="mx-auto max-w-[860px] px-5 pt-8 sm:px-7">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground">
      {copy.index.yoursTitle}
    </h1>
    <div class="flex flex-wrap items-center gap-1.5">
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
      <button
        type="button"
        class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        aria-pressed={editing}
        onclick={() => (editing = !editing)}>{editing ? h.done : h.edit}</button
      >
    </div>
  </div>

  {#if mounted && readingChoice.chosen === null && choiceOptions.length > 1}
    <!-- First-run choice (HOME-04): one line, browser-language default; the pick
         persists and the line never returns. -->
    <div class="mt-5 flex flex-wrap items-center gap-2">
      <span class="eyebrow">{h.readIn}</span>
      {#each choiceOptions as option (option.label)}
        <button
          type="button"
          class="flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          onclick={() => readingChoice.choose(option.choice)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  {/if}

  {#if mounted}
    <!-- EC-27 surface: the last durable-enqueue failure of the session sync
         outbox, never a silent drop. Cleared by the next successful enqueue. -->
    {#if sessions.enqueueError !== null}
      <p role="status" class="mt-5 text-[13px] text-danger">{h.sessionsSyncError}</p>
    {/if}
    <div class="mt-5 flex flex-wrap items-end gap-3">
      <label class="flex min-w-0 flex-1 flex-col gap-1 sm:max-w-[260px]">
        <span class="eyebrow">{h.sessionsLabel}</span>
        <select
          aria-label={h.sessionsLabel}
          value={activeSessionId}
          onchange={(event) => sessions.switchTo(event.currentTarget.value)}
          class="h-9 rounded-pill border border-border bg-transparent px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {#each sessionOptions as session (session.id)}
            <option value={session.id}>{session.name}</option>
          {/each}
        </select>
      </label>
      <form
        class="flex min-w-0 flex-1 items-end gap-2"
        onsubmit={(event) => {
          event.preventDefault();
          addSession();
        }}
      >
        <label class="flex min-w-0 flex-1 flex-col gap-1">
          <span class="sr-only">{h.sessionsNewPlaceholder}</span>
          <input
            type="text"
            placeholder={h.sessionsNewPlaceholder}
            value={newSessionName}
            maxlength="100"
            onchange={(event) => (newSessionName = event.currentTarget.value)}
            class="h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          />
        </label>
        <button
          type="submit"
          disabled={newSessionName.trim().length === 0}
          class="h-9 flex-none rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        >
          {h.sessionsNewAdd}
        </button>
      </form>
    </div>
  {/if}

  {#if editing}
    <div class="mt-5">
      <p class="eyebrow">{h.hidden}</p>
      <div class="mt-2 flex flex-wrap items-center gap-1.5">
        {#each widgets as widget (widget.id)}
          {#if !widget.visible}
            <button
              type="button"
              class="flex h-8 items-center gap-1 rounded-pill border border-dashed border-border-strong px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              onclick={() => homeLayout.toggle(widget.id)}
              >{widgetTitle(widget.id)}
              <span aria-hidden="true">+</span></button
            >
          {/if}
        {/each}
      </div>
    </div>
  {/if}

  {#if mounted}
  {#each widgets as widget (widget.id)}
  {#if widget.visible && widget.id === "continue-reading" && lastRead}
    {@const total = lastReadSurah?.ayahCount ?? 0}
    {@const fraction =
      reader.progressFor(lastRead.num) ?? (total > 0 ? Math.min(1, lastRead.n / total) : 0)}
    <section
      aria-labelledby="yours-continue"
      class="mt-5 overflow-hidden rounded-md border border-border bg-surface"
    >
      <h2
        id="yours-continue"
        class="eyebrow flex items-center justify-between gap-3 border-b border-border px-4 py-3 sm:px-5"
      >
        {copy.index.yoursContinue}
        {@render editControls(widget.id)}
      </h2>
      <div class="px-4 py-4 sm:px-5">
        <p class="text-xl font-medium text-foreground">
          {lastReadSurah?.name ?? `Surah ${lastRead.num}`}
        </p>
        <p class="mt-1 text-sm text-foreground-secondary">
          <span class="font-medium text-foreground">{lastRead.num}:{lastRead.n}</span>
          {#if lastReadSurah?.meaning}
            · {lastReadSurah.meaning}
          {/if}
          {#if chosenTranslationName}
            · {h.readingInName(chosenTranslationName)}
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
          <Button onclick={() => resumeToLastRead(continueCtx)} arrow>
            {copy.shell.jump}
          </Button>
        </div>
      </div>
    </section>
  {:else if widget.visible && widget.id === "continue-reading"}
    <div class="mt-5 flex items-center gap-3.5 rounded-md border border-border bg-surface px-4 py-4 sm:px-5">
      <span
        class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
        style:background="var(--hue-2-soft)"
        aria-hidden="true"
      >
        <Icon name="book" size={20} />
      </span>
      <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{copy.index.yoursEmpty}</p>
    </div>
  {:else if widget.visible && widget.id === "recents"}
    <section aria-labelledby="yours-recent" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-recent" class="eyebrow">{copy.index.yoursRecent}</h2>
        {#if editing}
          <label class="flex items-center gap-1.5 text-[12px] text-muted">
            {h.recentsCount}
            <select
              aria-label={h.recentsCount}
              value={recentsLimit}
              onchange={(event) =>
                homeLayout.setCount("recents", Number(event.currentTarget.value))}
              class="h-7 rounded-pill border border-border bg-transparent px-1.5 text-[12.5px] text-foreground-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
            >
              {#each RECENTS_COUNT_CHOICES as n (n)}
                <option value={n}>{n}</option>
              {/each}
            </select>
          </label>
        {/if}
        {@render editControls(widget.id)}
      </div>
      {#if recents.length === 0}
        <p class="mt-2 text-[13px] text-muted">{h.recentsEmpty}</p>
      {:else}
      <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
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
              <span class="text-[12.5px] text-muted">{recent.num}:{recent.n}</span>
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
      {/if}
    </section>
  {:else if widget.visible && widget.id === "bookmarks"}
    <section aria-labelledby="yours-bookmarks" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-bookmarks" class="eyebrow">{copy.nav.bookmarks}</h2>
        <span class="flex items-center gap-3 text-[12px] text-muted"
          >{bookmarkCount}{@render editControls(widget.id)}</span
        >
      </div>
      {#if bookmarkCount === 0}
        <div class="mt-2 flex items-center gap-3.5 rounded-md border border-border bg-surface px-4 py-4">
          <span
            class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
            style:background="var(--hue-2-soft)"
            aria-hidden="true"
          >
            <Icon name="bookmark" size={20} />
          </span>
          <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{h.bookmarksEmpty}</p>
        </div>
      {:else if bookmarkRows.length > 0}
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
          {#each bookmarkRows as row (row.key)}
            <!-- Shared row (BM-01): same component as /bookmarks. -->
            <BookmarkRow
              copy={rowCopy}
              bookmark={{
                id: row.key,
                folderId: null,
                surah: row.surah,
                ayah: row.ayah,
                updatedAt: row.updatedAt,
              }}
              surahName={row.name}
              href={row.href}
              noteIcon={row.hasNote}
              onRemove={() => removeBookmarkRow(row)}
            />
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
      {:else}
        <!-- count>0 but rows empty: names resolve only once quranData is in. Skeleton
             keeps the section from rendering no body mid-load (vanishing-sections fix). -->
        <ul
          class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface"
          aria-busy="true"
        >
          <li class="px-4 py-3"><Skeleton class="h-11 w-full" /></li>
          <li class="px-4 py-3"><Skeleton class="h-11 w-full" /></li>
        </ul>
      {/if}
    </section>
  {:else if widget.visible && widget.id === "notes"}
    <section aria-labelledby="yours-notes" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-notes" class="eyebrow">{h.notesTitle}</h2>
        <span class="flex items-center gap-3 text-[12px] text-muted"
          >{notesSource.rows.length}{@render editControls(widget.id)}</span
        >
      </div>
      {#if notesRows.length === 0}
        <p class="mt-2 text-[13px] text-muted">{h.notesEmpty}</p>
      {:else}
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
          {#each notesRows as note (note.key)}
            <li>
              <button
                type="button"
                class="flex w-full items-start gap-3 px-4 py-3 text-start transition-colors hover:bg-surface-hover"
                onclick={() => openNote(note)}
              >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium text-foreground"
                    >{quranData?.surahByNum(note.surah)?.name ?? `Surah ${note.surah}`}
                    <span class="font-normal text-muted"
                      >· {reader_bookmarks_ayah({ ayah: note.ayah }, msgOptions)}</span
                    ></span
                  >
                  <span class="mt-0.5 block truncate text-[12.5px] text-foreground-secondary"
                    >{firstNoteLine(note.text)}</span
                  >
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {:else if widget.visible && widget.id === "most-read"}
    <section aria-labelledby="yours-most-read" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-most-read" class="eyebrow">{h.mostReadTitle}</h2>
        {@render editControls(widget.id)}
      </div>
      {#if !consent.readingAnalytics}
        <p class="mt-2 text-[13px] text-muted">{h.mostReadOff}</p>
      {:else if mostRead.status === "ready"}
        <ul class="mt-2 divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
          {#each mostRead.top as entry (`${entry.surah}`)}
            {@const surah = quranData?.surahByNum(entry.surah)}
            {#if surah}
              <li class="flex items-center gap-3 px-4 py-3">
                <span class="truncate text-sm font-medium text-foreground">{surah.name}</span>
                <span class="ms-auto flex-none text-[12px] text-muted">{entry.reads}</span>
              </li>
            {/if}
          {/each}
        </ul>
      {:else}
        <p class="mt-2 text-[13px] text-muted">{h.mostReadEmpty}</p>
      {/if}
    </section>
  {:else if widget.visible && widget.id === "daily-ayah"}
    <section aria-labelledby="yours-daily-ayah" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-daily-ayah" class="eyebrow">{h.dailyAyahTitle}</h2>
        {@render editControls(widget.id)}
      </div>
      {#if dailyAyah}
        {@const surah = quranData?.surahByNum(dailyAyah.surah)}
        <div class="mt-2 rounded-md border border-border bg-surface px-4 py-4 sm:px-5">
          <p class="text-sm font-medium text-foreground">
            {surah?.name ?? `Surah ${dailyAyah.surah}`}
            <span class="font-normal text-muted">· {dailyAyah.surah}:{dailyAyah.ayah}</span>
          </p>
          <p class="mt-2 text-right text-[19px] leading-loose text-foreground" dir="rtl" lang="ar">
            {dailyAyah.text}
          </p>
          {#if surah}
            <div class="mt-3">
              <a
                href={publicHref(
                  readerHrefFor(copy.locale, surahAyahPathFor(ARABIC_CTX, surah, dailyAyah.ayah)),
                )}
                data-sveltekit-preload-data="hover"
                class="inline-flex h-8 items-center rounded-pill border border-border px-3 text-[12.5px] font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
              >
                {h.dailyAyahOpen}
                <span aria-hidden="true" class="ms-1">→</span>
              </a>
            </div>
          {/if}
        </div>
      {:else if dailyAyahLoaded}
        <p class="mt-2 text-[13px] text-muted">{h.dailyAyahUnavailable}</p>
      {:else}
        <p class="mt-2 text-[13px] text-muted" aria-busy="true">…</p>
      {/if}
    </section>
  {/if}
  {/each}
  {/if}

  <p class="mt-8 text-[11px] text-muted">
    <a href={publicHref(readerHrefFor(copy.locale, "/surah"))} class="underline-offset-4 hover:underline">
      {copy.seo.home}
    </a>
  </p>
</div>
