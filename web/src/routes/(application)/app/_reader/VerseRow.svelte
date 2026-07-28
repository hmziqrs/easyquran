<!--
  VerseRow — a single ayah inside the surah card. Ayah-number circle on the
  left, the Uthmani Arabic text (sized via reader.arabicSizePx — the one
  documented dynamic font-size exception), and a vertical stack of icon-only
  actions on the right: bookmark / play / note. When playing, the row takes the
  accent-soft tint (reader.rowHighlight). Expanding the note reveals the sample
  tafsir plus a per-verse Textarea bound through reader.getNote/setNote.
-->
<script lang="ts">
  import { reader } from "$lib/stores/reader.svelte";
  import { tafsirFor } from "$lib/data/quran";
  import { Icon } from "$lib/components/icon";
  import { Textarea } from "$lib/components/ui/textarea";
  import {
    Tooltip,
    TooltipTrigger,
    TooltipContent,
    TooltipProvider,
  } from "$lib/components/ui/tooltip";
  import { cn } from "$lib/utils";

  let { text, n, vKey }: { text: string; n: number; vKey: string } = $props();

  const tafsir = $derived(tafsirFor(vKey));
  const highlighted = $derived(reader.rowHighlight(vKey));
  const bookmarked = $derived(reader.isBookmarked(vKey));
  const playing = $derived(reader.isPlayingVerse(vKey));
  const noteOpen = $derived(reader.openNote === vKey);
  const hasNote = $derived(reader.getNote(vKey).length > 0);

  function onNote(e: Event) {
    reader.setNote(vKey, (e.currentTarget as HTMLTextAreaElement).value);
  }
</script>

<div class={cn("border-b border-line px-5 py-[26px] sm:px-9", highlighted && "bg-accent-soft")}>
  <div class="flex items-start gap-[22px]">
    <div
      class="flex-none flex h-[34px] w-[34px] items-center justify-center rounded-full border border-line text-[12.5px] text-fg-3"
    >
      {n}
    </div>
    <p
      dir="rtl"
      class="flex-1 min-w-0 text-right font-arabic leading-[2.15] text-fg"
      style="font-size:{reader.arabicSizePx}"
    >
      {text}
    </p>
    <div class="flex flex-none flex-col gap-1.5">
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                onclick={() => reader.toggleBookmark(vKey)}
                aria-label={bookmarked ? "Remove bookmark" : "Bookmark this verse"}
                class={cn(
                  "flex h-[30px] w-[30px] items-center justify-center rounded-lg transition-colors hover:bg-bg-2",
                  bookmarked ? "text-pop" : "text-fg-3",
                )}
              >
                <Icon name="bookmark" size={15} />
              </button>
            {/snippet}
          </TooltipTrigger>
          <TooltipContent>{bookmarked ? "Remove bookmark" : "Bookmark"}</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                onclick={() => reader.playVerse(vKey)}
                aria-label="Play this verse"
                class={cn(
                  "flex h-[30px] w-[30px] items-center justify-center rounded-lg transition-colors hover:bg-bg-2",
                  playing ? "text-accent" : "text-fg-3",
                )}
              >
                <Icon name="play" size={13} />
              </button>
            {/snippet}
          </TooltipTrigger>
          <TooltipContent>Play verse</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger>
            {#snippet child({ props })}
              <button
                {...props}
                type="button"
                onclick={() => reader.toggleNote(vKey)}
                aria-label={noteOpen ? "Close note and tafsir" : "Open note and tafsir"}
                class={cn(
                  "flex h-[30px] w-[30px] items-center justify-center rounded-lg transition-colors hover:bg-bg-2",
                  noteOpen || hasNote ? "text-accent" : "text-fg-3",
                )}
              >
                <Icon name="note" size={15} />
              </button>
            {/snippet}
          </TooltipTrigger>
          <TooltipContent>Note &amp; tafsir</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  </div>

  {#if noteOpen}
    <div class="ml-[56px] mt-[18px] flex flex-col gap-3.5 animate-fade-up">
      <div class="flex flex-col gap-1.5 rounded-[11px] bg-bg-2 px-[18px] py-4">
        <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-pop">Tafsir</span>
        <span class="text-[14.5px] leading-[1.65] text-fg-2">{tafsir}</span>
      </div>
      <div class="flex flex-col gap-2">
        <span class="text-[11.5px] font-semibold uppercase tracking-[0.1em] text-fg-3">Your note</span>
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
</div>
