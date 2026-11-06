<script lang="ts">
  import { onDestroy } from "svelte";
  import { page } from "$app/state";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import { toggleBookmark } from "#lib/bookmarks/feedback.svelte.js";
  import { parseVerseKey } from "#lib/bookmarks/schema.js";
  import {
    routeContextFromParams,
    translationIdFromSegments,
    type VerseKey,
  } from "#lib/data/quran.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { pins, PINS_MAX, type PinnedLane } from "#lib/stores/pins.svelte.js";
  import type { StackedTranslation } from "#lib/data/quran-types.js";
  import { Icon, type IconName } from "#lib/components/icon/index.js";
  import { Tooltip, TooltipTrigger, TooltipContent } from "#lib/components/ui/tooltip/index.js";
  import { Textarea } from "#lib/components/ui/textarea/index.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import {
    reader_bookmarks_undo,
    reader_reading_bookmark_here,
    reader_reading_bookmark_saved,
    reader_reading_bookmark_set,
    reader_pin_verse,
    reader_pins_full,
    reader_unpin_verse,
    reader_verse_note,
    reader_verse_note_close,
    reader_verse_note_open,
  } from "#lib/i18n/m/reader.js";
  import {
    currentReadingBookmark,
    restoreReadingBookmark,
    setReadingBookmark,
  } from "#lib/preferences/reading-bookmark.svelte.js";
  import { toast } from "svelte-sonner";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import { cn } from "#lib/utils.js";

  const COPY_FEEDBACK_MS = 1500;

  let {
    text,
    vKey,
    lanes = [],
  }: {
    text: string;
    vKey: string;
    /** The stacked translations this row shows; snapshotted when the verse is pinned. */
    lanes?: readonly StackedTranslation[];
  } = $props();

  const copy = getReaderUiCopy();
  const noteMsg = { locale: appLocale() };
  // SAFETY: vKey is the `surah:ayah` key of the rendered verse row, built from numeric surah and ayah.
  const verseKeyValue = $derived(vKey as VerseKey);
  // One store for everyone (device-local signed out, synced signed in).
  const bookmarked = $derived(bookmarks.isMarkedKey(vKey));
  const pinned = $derived(pins.has(vKey));
  const noteOpen = $derived(reader.openNote === verseKeyValue);
  const hasNote = $derived(reader.getNote(verseKeyValue).length > 0);

  // The translation being read (null on the Arabic reader). Bookmarks and notes
  // both record it so they reopen in the same translation.
  function currentSourceId(): string | null {
    const ctx = routeContextFromParams(page.params);
    return ctx.kind === "translation" ? translationIdFromSegments(ctx.lang, ctx.translator) : null;
  }

  // The reader's own place to return to. Automatic resume never touches it.
  const savedPlace = $derived(currentReadingBookmark());
  const isReadingBookmark = $derived(
    savedPlace !== null && `${savedPlace.surah}:${savedPlace.ayah}` === verseKeyValue,
  );

  function onSetReadingBookmark(): void {
    const coords = parseVerseKey(vKey);
    if (coords === null) return;
    const previous = setReadingBookmark({ ...coords, sourceId: currentSourceId() });
    toast(reader_reading_bookmark_saved(undefined, noteMsg), {
      id: "reading-bookmark",
      action: {
        label: reader_bookmarks_undo(undefined, noteMsg),
        onClick: () => restoreReadingBookmark(previous),
      },
    });
  // Pins keep the row's main text and the lanes it shows, device-local, in pin order.
  function onPin(): void {
    const result = pins.toggle({
      key: vKey,
      sourceId: currentSourceId(),
      text,
      lanes: lanes.map(
        (t): PinnedLane => ({ sourceId: t.sourceId, name: t.name ?? "", text: t.text }),
      ),
    });
    if (result === "full") {
      flashShareNote(reader_pins_full({ max: PINS_MAX }, { locale: copy.locale }));
    }
  }

  // Bookmarks reopen in the translation they were saved from.
  function onBookmark(): void {
    const coords = parseVerseKey(vKey);
    if (coords === null) return;
    toggleBookmark({ ...coords, sourceId: currentSourceId() });
  }

  // Notes autosave as you type; the note's source is the translation it was written in.
  function onNote(event: Event): void {
    // SAFETY: onNote is bound only to the Textarea oninput below; currentTarget is that textarea element.
    const value = (event.currentTarget as HTMLTextAreaElement).value;
    reader.setNote(verseKeyValue, value, currentSourceId());
  }

  let copied = $state(false);
  let sharedCopied = $state(false);
  let shareNote = $state("");
  let copyTimer: ReturnType<typeof setTimeout> | null = null;
  let shareTimer: ReturnType<typeof setTimeout> | null = null;
  let mounted = true;

  async function onCopy() {
    const ok = await reader.copyVerse(vKey, text);
    if (!ok || !mounted) return;
    copied = true;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => (copied = false), COPY_FEEDBACK_MS);
  }

  function flashShareNote(message: string) {
    if (!mounted) return;
    shareNote = message;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (shareNote = ""), 4000);
  }

  function flashSharedCopied() {
    if (!mounted) return;
    // Both feedback states share one timer; clear the note so a pending
    // failure timeout can't outlive a retry that succeeded.
    shareNote = "";
    sharedCopied = true;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (sharedCopied = false), COPY_FEEDBACK_MS);
  }

  async function onShare() {
    const result = await reader.shareVerse(vKey, text);
    if (result === "copied") {
      flashSharedCopied();
      return;
    }
    // FLOW-05: the "failed" result used to vanish — now it says what happened.
    if (result === "link-copied") flashShareNote(copy.verse.shareFailed);
    else if (result === "failed") flashShareNote(copy.verse.shareUnavailable);
  }

  onDestroy(() => {
    mounted = false;
    if (copyTimer) clearTimeout(copyTimer);
    if (shareTimer) clearTimeout(shareTimer);
  });
</script>

<!-- Constantly visible (user ask): the hover-gated toolbar hid the actions on
     desktop and made them feel unreachable. Muted ink keeps them quiet. -->
<!-- Touch screens (pointer: coarse) get 40px targets and a toolbar raised to clear the
     row's 60px text inset; 30px stays the fine-pointer size. -->
<div
  class="verse-toolbar absolute inset-x-0 top-[22px] flex items-center justify-between gap-2 opacity-100 [@media(pointer:coarse)]:top-3"
>
  <span class="text-[12.5px] text-foreground-secondary">{vKey}</span>
  <div class="flex items-center gap-0.5">
    {#snippet verseAction({ onclick, label, ariaLabel, icon, activeClass, pressed }: { onclick: (e: MouseEvent) => void; label: string; ariaLabel: string; icon: IconName; activeClass?: string; pressed?: boolean })}
      <Tooltip>
        <TooltipTrigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              onclick={onclick}
              aria-label={ariaLabel}
              aria-pressed={pressed}
              class={cn(
                "flex size-[30px] touch-manipulation items-center justify-center rounded-md transition-colors hover:bg-surface-hover [@media(pointer:coarse)]:size-10",
                activeClass ?? "text-foreground-secondary hover:text-foreground",
              )}
            >
              <Icon name={icon} size={15} />
            </button>
          {/snippet}
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    {/snippet}

    {#if shareNote}
      <!-- Share failure feedback (FLOW-05): a live region so the outcome is
           announced, not just the silent icon swap the old code had. -->
      <span role="status" class="share-note">{shareNote}</span>
    {/if}
    {@render verseAction({
      onclick: onPin,
      label: pinned ? reader_unpin_verse(undefined, noteMsg) : reader_pin_verse(undefined, noteMsg),
      ariaLabel: pinned ? reader_unpin_verse(undefined, noteMsg) : reader_pin_verse(undefined, noteMsg),
      icon: "pin",
      activeClass: pinned ? "text-primary" : undefined,
      pressed: pinned,
    })}
    {@render verseAction({
      onclick: onBookmark,
      label: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmark,
      ariaLabel: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmarkVerse,
      icon: bookmarked ? "bookmark-fill" : "bookmark",
      activeClass: bookmarked ? "text-primary" : undefined,
      pressed: bookmarked,
    })}
    {@render verseAction({
      onclick: onSetReadingBookmark,
      label: reader_reading_bookmark_set(undefined, noteMsg),
      ariaLabel: isReadingBookmark
        ? reader_reading_bookmark_here(undefined, noteMsg)
        : reader_reading_bookmark_set(undefined, noteMsg),
      icon: "book",
      activeClass: isReadingBookmark ? "text-primary" : undefined,
      pressed: isReadingBookmark,
    })}
    {@render verseAction({
      onclick: onCopy,
      label: copied ? copy.verse.copied : copy.verse.copy,
      ariaLabel: copy.verse.copyAyah,
      icon: copied ? "check" : "copy",
      activeClass: copied ? "text-primary" : undefined,
    })}
    {@render verseAction({
      onclick: onShare,
      label: sharedCopied ? copy.verse.copied : copy.verse.share,
      ariaLabel: copy.verse.shareVerse,
      icon: sharedCopied ? "check" : "share",
      activeClass: sharedCopied ? "text-primary" : undefined,
    })}
    {@render verseAction({
      onclick: () => reader.toggleNote(verseKeyValue),
      label: reader_verse_note(undefined, noteMsg),
      ariaLabel: noteOpen
        ? reader_verse_note_close(undefined, noteMsg)
        : reader_verse_note_open(undefined, noteMsg),
      icon: "note",
      activeClass: noteOpen || hasNote ? "text-primary" : undefined,
      pressed: noteOpen,
    })}
  </div>
</div>

{#if noteOpen}
  <div class="verse-note mt-[20px] flex flex-col gap-2 animate-fade-up">
    <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
      {copy.verse.yourNote}
    </span>
    <Textarea
      value={reader.getNote(verseKeyValue)}
      oninput={onNote}
      rows={3}
      aria-label={copy.verse.yourNote}
      placeholder={copy.verse.notePlaceholder}
      class="resize-y rounded-md border-border bg-surface px-3.5 py-3 text-[14.5px] leading-[1.6] text-foreground"
    />
    <span class="text-[12.5px] text-muted-foreground">{copy.verse.noteSaved}</span>
  </div>
{/if}

<style>
  :global([data-reader-mode="reading"]) .verse-toolbar,
  :global([data-reader-mode="reading"]) .verse-note {
    display: none;
  }

  /* Share-failure note rides just above the toolbar, inside the row's 60px
     top inset; nowrap is safe — the message is short and rows are full-width. */
  .share-note {
    bottom: calc(100% + 2px);
    color: var(--muted);
    font-size: 12px;
    inset-inline-end: 0;
    position: absolute;
    white-space: nowrap;
  }
</style>
