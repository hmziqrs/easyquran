<script lang="ts">
  import { onDestroy } from "svelte";
  import { bookmarks } from "$lib/bookmarks/store.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { Icon, type IconName } from "$lib/components/icon";
  import { Tooltip, TooltipTrigger, TooltipContent } from "$lib/components/ui/tooltip";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { cn } from "$lib/utils";

  const COPY_FEEDBACK_MS = 1500;

  let { text, vKey }: { text: string; vKey: string } = $props();

  const copy = getReaderUiCopy();
  // Unified view: authed sessions read the synced bookmarks store, anonymous ones the legacy local map.
  const bookmarked = $derived(bookmarks.isMarkedKey(vKey));

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

  onDestroy(() => {
    mounted = false;
    if (copyTimer) clearTimeout(copyTimer);
    if (shareTimer) clearTimeout(shareTimer);
  });
</script>

<!-- Constantly visible (user ask): the hover-gated toolbar hid the actions on
     desktop and made them feel unreachable. Muted ink keeps them quiet. -->
<div
  class="verse-toolbar absolute inset-x-0 top-[22px] flex items-center justify-between gap-2 opacity-100"
>
  <span class="font-mono text-[11.5px] tracking-wide text-foreground-secondary">{vKey}</span>
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
  </div>
</div>

<style>
  :global([data-reader-mode="reading"]) .verse-toolbar,
  :global([data-reader-mode="reading"]) .verse-note {
    display: none;
  }
</style>
