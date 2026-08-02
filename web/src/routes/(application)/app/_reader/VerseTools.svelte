<script lang="ts">
  import { onDestroy } from "svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { tafsirFor } from "$lib/data/quran";
  import { Icon, type IconName } from "$lib/components/icon";
  import { Textarea } from "$lib/components/ui/textarea";
  import { Tooltip, TooltipTrigger, TooltipContent } from "$lib/components/ui/tooltip";
  import { cn } from "$lib/utils";

  let { text, vKey }: { text: string; vKey: string } = $props();

  const tafsir = $derived(tafsirFor(vKey));
  const bookmarked = $derived(reader.isBookmarked(vKey));
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
    copyTimer = setTimeout(() => (copied = false), 1500);
  }

  async function onShare() {
    const result = await reader.shareVerse(vKey, text);
    if (result !== "copied" || !mounted) return;
    sharedCopied = true;
    if (shareTimer) clearTimeout(shareTimer);
    shareTimer = setTimeout(() => (sharedCopied = false), 1500);
  }

  function onNote(event: Event) {
    reader.setNote(vKey, (event.currentTarget as HTMLTextAreaElement).value);
  }

  onDestroy(() => {
    mounted = false;
    if (copyTimer) clearTimeout(copyTimer);
    if (shareTimer) clearTimeout(shareTimer);
  });
</script>

<div
  class="verse-toolbar mb-2.5 flex items-center justify-between gap-2 opacity-100 md:opacity-0 md:group-hover:opacity-100 md:group-focus-within:opacity-100"
>
  <span class="font-mono text-[11px] tracking-wide text-fg-3">{vKey}</span>
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
                "flex h-[30px] w-[30px] items-center justify-center rounded-lg transition-colors hover:bg-bg-2",
                activeClass ?? "text-fg-3 hover:text-fg",
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
      onclick: () => reader.toggleBookmark(vKey),
      label: bookmarked ? "Remove bookmark" : "Bookmark",
      ariaLabel: bookmarked ? "Remove bookmark" : "Bookmark this verse",
      icon: "bookmark",
      activeClass: bookmarked ? "text-pop" : undefined,
    })}
    {@render verseAction({
      onclick: onCopy,
      label: copied ? "Copied" : "Copy",
      ariaLabel: "Copy ayah",
      icon: copied ? "check" : "copy",
      activeClass: copied ? "text-accent" : undefined,
    })}
    {@render verseAction({
      onclick: onShare,
      label: sharedCopied ? "Copied" : "Share",
      ariaLabel: "Share verse",
      icon: sharedCopied ? "check" : "share",
      activeClass: sharedCopied ? "text-accent" : undefined,
    })}
    {@render verseAction({
      onclick: () => reader.toggleNote(vKey),
      label: "Note & tafsir",
      ariaLabel: noteOpen ? "Close note and tafsir" : "Open note and tafsir",
      icon: "note",
      activeClass: noteOpen || hasNote ? "text-accent" : undefined,
    })}
  </div>
</div>

{#if noteOpen}
  <div class="verse-note mt-[20px] flex flex-col gap-3.5 animate-fade-up">
    <div class="flex flex-col gap-1.5 rounded-[11px] bg-bg-2 px-[18px] py-4">
      <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-pop">Tafsir</span>
      <span class="text-[14.5px] leading-[1.65] text-fg-2">{tafsir}</span>
    </div>
    <div class="flex flex-col gap-2">
      <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-fg-3">
        Your note
      </span>
      <Textarea
        value={reader.getNote(vKey)}
        oninput={onNote}
        rows={3}
        aria-label="Your note"
        placeholder="Write a reflection…"
        class="resize-y rounded-[11px] border-line bg-bg-3 px-3.5 py-3 text-[14.5px] leading-[1.6] text-fg"
      />
      <span class="text-[12.5px] text-fg-3">Saved on this device as you type.</span>
    </div>
  </div>
{/if}

<style>
  :global([data-reader-mode="reading"]) .verse-toolbar,
  :global([data-reader-mode="reading"]) .verse-note {
    display: none;
  }
</style>
