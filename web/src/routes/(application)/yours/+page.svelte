<script lang="ts">
  import { onMount } from "svelte";
  import { loadQuranData, peekQuranData } from "#lib/data/quran-data-client.js";
  import type { QuranData } from "#lib/data/quran-data.js";
  import {
    surahAyahPathFor,
    surahRouteContext,
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
  import { verseKeyOf, type Bookmark } from "#lib/bookmarks/schema.js";
  import BookmarkRow from "../bookmarks/_components/BookmarkRow.svelte";
  import { openVerseAt } from "#lib/reader/resume.js";
  import { homeLayout, type HomeWidgetId } from "#lib/home/layout.svelte.js";
  import { entryAttrs } from "#lib/sessions/entry.js";
  import ContinueThreads from "./_components/ContinueThreads.svelte";
  import ThreadHistory from "./_components/ThreadHistory.svelte";
  import ReadingInsights from "./_components/ReadingInsights.svelte";
  import { homeCards, type HomeCard } from "#lib/home/cards.svelte.js";
  import CardTile from "#lib/home/CardTile.svelte";
  import CardForm from "#lib/home/CardForm.svelte";
  import { cardUndoState, undoCardRemoval } from "#lib/home/card-undo.svelte.js";
  import { fetchDailyAyah, type DailyAyah } from "#lib/home/daily-ayah.js";
  import { LOCAL_NOTES_OWNER, notesSource } from "#lib/home/notes-source.svelte.js";
  import {
    languageLabelFor,
    readingChoice,
    suggestedChoice,
    translationForLanguage,
    type ReadingChoice,
  } from "#lib/home/reading-choice.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { peekTranslationName } from "#lib/quran/catalogue.js";
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
    reader_widget_daily_ayah_open,
    reader_widget_daily_ayah_title,
    reader_widget_daily_ayah_unavailable,
    reader_widget_most_read_title,
    reader_widget_notes_empty,
    reader_widget_notes_title,
    reader_cards_title,
    reader_threads_history,
    reader_cards_empty,
    reader_cards_add,
    reader_cards_full,
    reader_cards_removed,
    reader_bookmarks_undo,
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
    bookmarksEmpty: reader_bookmarks_empty_page(undefined, msgOptions),
    notesTitle: reader_widget_notes_title(undefined, msgOptions),
    notesEmpty: reader_widget_notes_empty(undefined, msgOptions),
    mostReadTitle: reader_widget_most_read_title(undefined, msgOptions),
    dailyAyahTitle: reader_widget_daily_ayah_title(undefined, msgOptions),
    dailyAyahUnavailable: reader_widget_daily_ayah_unavailable(undefined, msgOptions),
    dailyAyahOpen: reader_widget_daily_ayah_open(undefined, msgOptions),
    readIn: reader_home_read_in(undefined, msgOptions),
    readInArabic: reader_home_read_in_arabic(undefined, msgOptions),
    readInWith: (language: string): string =>
      reader_home_read_in_with({ language }, msgOptions),
    historyTitle: reader_threads_history(undefined, msgOptions),
    cards: {
      title: reader_cards_title(undefined, msgOptions),
      empty: reader_cards_empty(undefined, msgOptions),
      add: reader_cards_add(undefined, msgOptions),
      full: reader_cards_full(undefined, msgOptions),
    },
  };

  // The widget column renders only after mount: SSR would paint the default
  // layout and reorder on hydrate (the stored layout is device-local).
  let mounted = $state(false);
  let editing = $state(false);

  const widgets = $derived(homeLayout.entries);

  function widgetTitle(id: HomeWidgetId): string {
    if (id === "continue-reading") return copy.index.yoursContinue;
    if (id === "recents") return h.historyTitle;
    if (id === "notes") return h.notesTitle;
    if (id === "most-read") return h.mostReadTitle;
    if (id === "daily-ayah") return h.dailyAyahTitle;
    if (id === "cards") return h.cards.title;
    return copy.nav.bookmarks;
  }

  /* ── Widget cards (pinned surah/ayah/range, zikr, azkar sets, collections) ──
     The add/edit form lives in CardForm.svelte; this section owns which form
     is open ("new" | the card being edited | none), the responsive tile grid
     and the removal undo banner. Only one form is open at a time. */
  let formCard = $state<"new" | HomeCard | null>(null);
  let addCardButton = $state<HTMLElement | null>(null);
  let undoButton = $state<HTMLButtonElement | null>(null);

  const undo = $derived(cardUndoState());

  // An armed banner (fresh removal or an arrival from the detail page) takes
  // focus once, so the destructive action is undoable from the keyboard.
  $effect(() => {
    if (undo !== null) undoButton?.focus();
  });

  function openAddForm(): void {
    formCard = "new";
  }

  function openEditForm(card: HomeCard): void {
    formCard = card;
  }

  // Cancel and successful Save both land here: close + focus back to the opener.
  function closeCardForm(): void {
    formCard = null;
    addCardButton?.focus();
  }

  function undoRemove(): void {
    undoCardRemoval();
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

  // Continue / History resume in the reader's chosen translation unless a
  // thread carries its own (resumeCtxFor prefers the thread's sourceId).
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
    readonly bookmark: Bookmark;
    readonly name: string;
    readonly href: string;
    readonly hasNote: boolean;
  }

  // Opening a saved verse starts a reading thread there.
  const BOOKMARK_ENTRY = entryAttrs({ origin: "bookmark", exact: true });

  const bookmarkCount = $derived(bookmarks.bookmarks.length);

  const bookmarkRows = $derived.by<BookmarkPreviewRow[]>(() => {
    const data = quranData;
    if (!data) return [];
    return bookmarks.bookmarks.slice(0, 5).flatMap((bookmark) => {
      const surah = data.surahByNum(bookmark.surah);
      if (!surah) return [];
      return [
        {
          bookmark,
          name: surah.name,
          href: surahAyahPathFor(ctxForSavedSource(bookmark.sourceId), surah, bookmark.ayah),
          hasNote: reader.getNote(verseKeyOf(bookmark.surah, bookmark.ayah)).trim().length > 0,
        },
      ];
    });
  });

  function openNote(note: { surah: number; ayah: number }): void {
    void openVerseAt(note.surah, note.ayah, undefined, ARABIC_CTX);
  }

  /** BM-03 rows show the note's first line; notes are plain strings (no dates). */
  function firstNoteLine(text: string): string {
    return text.split("\n")[0] ?? text;
  }

  onMount(() => {
    mounted = true;
    homeLayout.hydrate();
    homeCards.hydrate();
    readingChoice.hydrate();
    if (readingChoice.chosen === null) buildChoiceOptions();
    reader.hydrate();
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
  {#if widget.visible && widget.id === "continue-reading"}
    <section aria-labelledby="yours-continue" class="mt-5">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-continue" class="eyebrow">{copy.index.yoursContinue}</h2>
        {@render editControls(widget.id)}
      </div>
      <ContinueThreads {quranData} ctx={continueCtx} {locale} continueLabel={copy.shell.jump} />
    </section>
  {:else if widget.visible && widget.id === "recents"}
    <section aria-labelledby="yours-recent" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-recent" class="eyebrow">{h.historyTitle}</h2>
        {@render editControls(widget.id)}
      </div>
      <ThreadHistory {quranData} ctx={continueCtx} {locale} limit={recentsLimit} />
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
          {#each bookmarkRows as row (row.bookmark.id)}
            <!-- Shared row: same component as /bookmarks, folder badge included. -->
            <BookmarkRow
              copy={rowCopy}
              bookmark={{ ...row.bookmark, updatedAt: row.bookmark.createdAt }}
              surahName={row.name}
              href={row.href}
              linkAttrs={BOOKMARK_ENTRY}
              folder={bookmarks.folderById(row.bookmark.folderId) ?? null}
              noteIcon={row.hasNote}
              onRemove={() => bookmarks.remove(row.bookmark.id)}
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
      <ReadingInsights {quranData} {locale} />
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
  {:else if widget.visible && widget.id === "cards"}
    <section aria-labelledby="yours-cards" class="mt-6">
      <div class="flex items-center justify-between gap-3">
        <h2 id="yours-cards" class="eyebrow">{h.cards.title}</h2>
        <span class="flex items-center gap-3 text-[12px] text-muted"
          >{homeCards.list.length}{@render editControls(widget.id)}</span
        >
      </div>

      {#if undo !== null}
        <!-- Removal undo (bookmarks pattern): optimistic remove + 5s window.
             On a refused restore the banner stays armed, the window restarts
             and the refusal line shows inside it. -->
        <div
          role="status"
          class="mt-2 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border-strong bg-background-subtle px-4 py-2.5"
        >
          <p class="text-caption text-foreground-secondary">
            {reader_cards_removed(undefined, msgOptions)}
            {#if undo.failed}
              <span role="alert" class="ms-2 font-medium text-danger">{h.cards.full}</span>
            {/if}
          </p>
          <Button
            type="button"
            bind:ref={undoButton}
            variant="secondary"
            size="sm"
            class="h-8 px-3 text-[12.5px]"
            onclick={undoRemove}
          >
            {reader_bookmarks_undo(undefined, msgOptions)}
          </Button>
        </div>
      {/if}

      {#if formCard !== null}
        {#key formCard === "new" ? "new" : formCard.id}
          <div class="mt-3">
            <CardForm
              quranData={quranData}
              initial={formCard === "new" ? undefined : formCard}
              onClose={closeCardForm}
            />
          </div>
        {/key}
      {:else if homeCards.full}
        <p class="mt-2 text-[13px] text-muted">{h.cards.full}</p>
      {/if}

      {#if homeCards.list.length === 0 && formCard === null}
        <!-- Composed empty state: no dead end — the CTA opens the add form. -->
        <div class="mt-2 flex flex-col gap-3 rounded-md border border-border bg-surface px-4 py-4">
          <div class="flex items-center gap-3.5">
            <span
              class="flex size-11 flex-none items-center justify-center rounded-sm text-[var(--hue-2-legible)]"
              style:background="var(--hue-2-soft)"
              aria-hidden="true"
            >
              <Icon name="plus" size={20} />
            </span>
            <p class="text-[14.5px] leading-relaxed text-foreground-secondary">{h.cards.empty}</p>
          </div>
          <div>
            <Button bind:ref={addCardButton} onclick={openAddForm}>{h.cards.add}</Button>
          </div>
        </div>
      {:else if homeCards.list.length > 0}
        <div class="mt-2 grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {#each homeCards.list as card (card.id)}
            <CardTile {card} quranData={quranData} menu onEdit={openEditForm} />
          {/each}
        </div>
        {#if formCard === null && homeCards.full === false}
          <div class="mt-3">
            <Button bind:ref={addCardButton} onclick={openAddForm}>{h.cards.add}</Button>
          </div>
        {/if}
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
