<script lang="ts">
  import { onDestroy } from "svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { tafsirFor } from "$lib/data/quran";
  import { Icon, type IconName } from "$lib/components/icon";
  import { Textarea } from "$lib/components/ui/textarea";
  import { Tooltip, TooltipTrigger, TooltipContent } from "$lib/components/ui/tooltip";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { cn } from "$lib/utils";

  const COPY_FEEDBACK_MS = 1500;

  let {
    text,
    vKey,
    onToggleNote,
  }: { text: string; vKey: string; onToggleNote?: () => void } = $props();

  const tafsir = $derived(tafsirFor(vKey));
  const copy = getReaderUiCopy();
  // Unified view: authed sessions read the synced bookmarks store, anonymous ones the legacy local map.
  const bookmarked = $derived(bookmarks.isMarkedKey(vKey));
  const noteOpen = $derived(reader.openNote === vKey);
  const hasNote = $derived(reader.getNote(vKey).length > 0);

  let copied = $state(false);
  let sharedCopied = $state(false);
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

  async function onShare() {
    const result = await reader.shareVerse(vKey, text);
    if (result !== "copied" || !mounted) return;
    sharedCopied = true;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (sharedCopied = false), COPY_FEEDBACK_MS);
  }

  function onNote(event: Event) {
    // SAFETY: onNote is bound only to the Textarea oninput below; currentTarget is that textarea element.
    reader.setNote(vKey, (event.currentTarget as HTMLTextAreaElement).value);
  }

  onDestroy(() => {
    mounted = false;
    if (copyTimer) clearTimeout(copyTimer);
    if (shareTimer) clearTimeout(shareTimer);
  });
</script>

<!-- Constantly visible (user ask): the hover-gated toolbar hid the actions on
     desktop and made them feel unreachable. Muted ink keeps them quiet. -->
<div
  class="verse-toolbar absolute inset-x-5 top-[22px] flex items-center justify-between gap-2 opacity-100 sm:inset-x-9"
>
  <span class="font-mono text-[11px] tracking-wide text-muted-foreground">{vKey}</span>
  <div class="flex items-center gap-0.5">
    {#snippet verseAction({ onclick, label, ariaLabel, icon, activeClass }: { onclick: (e: MouseEvent) => void; label: string; ariaLabel: string; icon: IconName; activeClass?: string })}
      <Tooltip>
        <TooltipTrigger>
          {#snippet child({ props })}
            <button
              {...props}
              type="button"
              onclick={onclick}
              aria-label={ariaLabel}
              class={cn(
                "flex h-[30px] w-[30px] items-center justify-center rounded-md transition-colors hover:bg-surface-hover",
                activeClass ?? "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon name={icon} size={15} />
            </button>
          {/snippet}
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    {/snippet}

    {@render verseAction({
      onclick: () => bookmarks.toggleVerse(vKey),
      label: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmark,
      ariaLabel: bookmarked ? copy.verse.removeBookmark : copy.verse.bookmarkVerse,
      icon: "bookmark",
      activeClass: bookmarked ? "text-primary" : undefined,
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
      onclick: () => (onToggleNote ? onToggleNote() : reader.toggleNote(vKey)),
      label: copy.verse.noteTafsir,
      ariaLabel: noteOpen ? copy.verse.closeNoteTafsir : copy.verse.openNoteTafsir,
      icon: "note",
      activeClass: noteOpen || hasNote ? "text-primary" : undefined,
    })}
  </div>
</div>

{#if noteOpen}
  <div class="verse-note mt-[20px] flex flex-col gap-3.5 animate-fade-up">
    <div class="flex flex-col gap-1.5 rounded-md bg-background-subtle px-[18px] py-4">
      <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-muted-foreground"
        >{copy.verse.tafsir}</span
      >
      <span class="text-[14.5px] leading-[1.65] text-foreground-secondary">{tafsir}</span>
    </div>
    <div class="flex flex-col gap-2">
      <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-muted-foreground">
        {copy.verse.yourNote}
      </span>
      <Textarea
        value={reader.getNote(vKey)}
        oninput={onNote}
        rows={3}
        aria-label={copy.verse.yourNote}
        placeholder={copy.verse.notePlaceholder}
        class="resize-y rounded-md border-border bg-surface px-3.5 py-3 text-[14.5px] leading-[1.6] text-foreground"
      />
      <span class="text-[12.5px] text-muted-foreground">{copy.verse.noteSaved}</span>
    </div>
  </div>
{/if}

<style>
  :global([data-reader-mode="reading"]) .verse-toolbar,
  :global([data-reader-mode="reading"]) .verse-note {
    display: none;
  }
</style>
