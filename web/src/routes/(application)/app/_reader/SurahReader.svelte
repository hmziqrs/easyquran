<script lang="ts">
  import { resolve } from "$app/paths";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import {
    surahMeta,
    verseKey,
    toArabicDigits,
    surahPath,
    type Surah,
  } from "$lib/data/quran";
  import type { SurahLink } from "$lib/data/quran-types";
  import { Icon } from "$lib/components/icon";
  import VerseRow from "./VerseRow.svelte";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import * as Tabs from "$lib/components/ui/tabs";
  import { displayVerses, headerText } from "$lib/quran/view/presentation";

  let {
    surah,
    previous,
    next,
  }: { surah: Surah; previous?: SurahLink; next?: SurahLink } = $props();

  const verses = $derived(displayVerses(surah));
  const opener = $derived(headerText(surah.normalization));
  const badge = $derived(String(surah.num).padStart(3, "0"));

  function continueReading() {
    const lr = reader.lastRead;
    if (lr) reader.openVerse(lr.num, lr.n);
  }
</script>

<div class="flex flex-col gap-4">
  {#if reader.hasLastRead}
    <button
      type="button"
      onclick={continueReading}
      class="flex items-center gap-3 rounded-[12px] bg-accent-soft px-[18px] py-[13px] text-left transition-[filter] duration-150 hover:brightness-[0.98]"
    >
      <Icon name="play" size={15} class="flex-none text-accent" />
      <span class="text-sm text-accent">Continue reading — {reader.lastReadRef}</span>
      <span class="ml-auto text-[13px] text-accent/75">Jump →</span>
    </button>
  {/if}

  <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
    <Tabs.Root value={reader.mode} onValueChange={(v) => reader.setMode(v as ReaderMode)}>
    <div
      class="flex flex-wrap items-start justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:px-9"
    >
      <div class="flex items-start gap-4">
        <div
          aria-hidden="true"
          class="flex h-16 w-16 flex-none items-center justify-center rounded-2xl border border-accent bg-accent-soft font-arabic text-lg text-accent"
        >
          {badge}
        </div>
        <div class="flex min-w-0 flex-col gap-1.5">
          <span class="text-xs font-semibold uppercase tracking-[0.1em] text-accent">
            Surah {surah.num}
          </span>
          <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
            <h1 class="text-[32px] font-semibold tracking-[-0.025em]">
              {surah.num}. {surah.name}
            </h1>
            <span dir="rtl" class="font-arabic text-[30px] leading-none text-fg-2">
              {surah.arabic}
            </span>
          </div>
          <span class="text-sm text-fg-3">{surahMeta(surah)}</span>
        </div>
      </div>

      <div class="flex flex-wrap items-center justify-end gap-2">
        <div
          class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
          role="group"
          aria-label="Arabic text size"
        >
          <button
            type="button"
            onclick={() => reader.smaller()}
            aria-label="Smaller Arabic text"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A&minus;
          </button>
          <button
            type="button"
            onclick={() => reader.bigger()}
            aria-label="Larger Arabic text"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A+
          </button>
        </div>

        <Tabs.List
          aria-label="Reading mode"
          class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
        >
          <Tabs.Trigger
            value="verse"
            class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors data-[state=active]:bg-bg-3 data-[state=active]:text-fg data-[state=inactive]:text-fg-3 data-[state=inactive]:hover:text-fg"
          >
            <Icon name="rows" size={13} />
            <span class="hidden sm:inline">Ayah-by-Ayah</span>
            <span class="sm:hidden">Ayahs</span>
          </Tabs.Trigger>
          <Tabs.Trigger
            value="reading"
            class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors data-[state=active]:bg-bg-3 data-[state=active]:text-fg data-[state=inactive]:text-fg-3 data-[state=inactive]:hover:text-fg"
          >
            <Icon name="continuous" size={13} />
            <span>Reading</span>
          </Tabs.Trigger>
        </Tabs.List>
      </div>
    </div>

    {#if opener}
      <p dir="rtl" class="py-2 text-center font-arabic text-fg-3">{opener}</p>
    {/if}

    <Tabs.Content
      value="verse"
      class="focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-line-3"
    >
      <TooltipProvider delayDuration={300}>
        <div class="flex flex-col">
          {#each verses as text, i (verseKey(surah.num, i + 1))}
            <VerseRow text={text} n={i + 1} vKey={verseKey(surah.num, i + 1)} />
          {/each}
        </div>
      </TooltipProvider>
    </Tabs.Content>
    <Tabs.Content
      value="reading"
      class="focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-line-3"
    >
      <div
        dir="rtl"
        class="reading-text px-5 py-8 text-fg sm:px-9"
        style="font-size:{reader.arabicSizePx}"
      >{#each verses as text, i (verseKey(surah.num, i + 1))}<span>{text}</span><span id="ayah-{i + 1}" class="ayah-marker">{toArabicDigits(i + 1)}</span> {/each}</div>
    </Tabs.Content>
    </Tabs.Root>

    <div class="flex items-center justify-between gap-4 px-5 py-[22px] sm:px-9">
      {#if previous}
        <a
          href={resolve(surahPath(previous))}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          <span aria-hidden="true">←</span>
          {previous.name}
        </a>
      {:else}
        <span></span>
      {/if}
      {#if next}
        <a
          href={resolve(surahPath(next))}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {next.name}
          <span aria-hidden="true">→</span>
        </a>
      {/if}
    </div>
  </div>
</div>
