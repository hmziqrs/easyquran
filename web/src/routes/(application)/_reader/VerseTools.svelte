<script lang="ts">
  import { onDestroy } from "svelte";
  import { bookmarks } from "#lib/bookmarks/store.svelte.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { Icon, type IconName } from "#lib/components/icon/index.js";
  import { Tooltip, TooltipTrigger, TooltipContent } from "#lib/components/ui/tooltip/index.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { cn } from "#lib/utils.js";

  const COPY_FEEDBACK_MS = 1500;

  let { text, vKey }: { text: string; vKey: string } = $props();

  const copy = getReaderUiCopy();
  // Unified view: authed sessions read the synced bookmarks store, anonymous ones the legacy local map.
  const bookmarked = $derived(bookmarks.isMarkedKey(vKey));

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
      onclick: () => bookmarks.toggleVerse(vKey),
      label: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmark,
      ariaLabel: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmarkVerse,
      icon: bookmarked ? "bookmark-fill" : "bookmark",
      activeClass: bookmarked ? "text-primary" : undefined,
      pressed: bookmarked,
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
  </div>
</div>

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
