<script lang="ts">
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import { surahMeta, type SurahLocalPageData } from "$lib/data/quran";
  import { Icon } from "$lib/components/icon";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import type { ReadingText } from "$lib/stores/reading-text.svelte";

  let {
    initial,
    visibleLocalPage,
    clientMounted,
    onChangeMode,
    readingText,
    readingFlowName,
    onReadArabic,
    onReadTranslation,
    onPickTranslation,
  }: {
    initial: SurahLocalPageData;
    visibleLocalPage: number;
    clientMounted: boolean;
    onChangeMode: (mode: ReaderMode) => void;
    /** What Reading flows right now. */
    readingText: ReadingText;
    /** Name of the translation flowing in Reading, if any. */
    readingFlowName: string | null;
    onReadArabic: () => void;
    /** Flow the last-read translation (or open the picker when there is none yet). */
    onReadTranslation: () => void;
    /** Open the full Translations picker to choose the text Reading flows. */
    onPickTranslation: () => void;
  } = $props();

  const copy = getReaderUiCopy();

  // Mode pills sit in an outlined well on the reading ground; the active pill is
  // ground-inverted (fg on bg), never the palette accent.
  // On phones the pills share the row evenly; from sm up they size to their labels.
  const pill =
    "flex h-8 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-sm px-3 text-[13px] font-medium transition-colors text-foreground-secondary hover:text-foreground aria-pressed:bg-foreground aria-pressed:text-background aria-pressed:hover:text-background sm:flex-none";
</script>

<!-- Strip header (user pick, /design/mix head=e): one compact row — number, name, Arabic
     name, meta — so the text starts sooner. No hue band; A−/A+ live in the sticky bar. -->
<div class="flex flex-wrap items-center gap-x-4 gap-y-3 border-b border-reader-divider py-4">
  <span
    aria-hidden="true"
    class="flex size-9 flex-none items-center justify-center rounded-pill bg-primary text-[13px] font-semibold tabular-nums text-primary-foreground"
  >
    {initial.surah.num}
  </span>
  <div class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-3 gap-y-0.5">
    <h1 class="text-[21px] font-semibold tracking-[-0.015em] text-foreground">
      <span class="sr-only">{initial.surah.num}. </span>{initial.surah.name}
    </h1>
    <span dir="rtl" lang="ar" class="font-arabic text-[24px] leading-none text-foreground">
      {initial.surah.arabic}
    </span>
    <span class="text-[13.5px] text-foreground-secondary">
      {surahMeta(initial.surah)} · {copy.shell.pageOf(visibleLocalPage, initial.pageCount)}
    </span>
  </div>

  {#if clientMounted}
    <div
      class="flex w-full items-center gap-0.5 rounded-md border border-border p-0.5 sm:ms-auto sm:w-auto"
      role="group"
      aria-label={copy.shell.readingModeLabel}
    >
      <button
        type="button"
        aria-pressed={reader.isVerseMode}
        onclick={() => onChangeMode("verse")}
        class={pill}
      >
        <Icon name="rows" size={13} />
        <span class="hidden sm:inline">{copy.shell.ayahByAyah}</span>
        <span class="sm:hidden">{copy.shell.ayahs}</span>
      </button>
      {#if reader.isReadingMode}
        <!-- Reading splits into its two texts (quran.com's model): no dialog, one tap. -->
        <button type="button" aria-pressed={readingText === "arabic"} onclick={onReadArabic} class={pill}>
          <Icon name="continuous" size={13} />
          <span>{copy.shell.readingArabic}</span>
        </button>
        <!-- Translation: one tap flows the last translation; tapping it again (or ▾) opens the
             full Translations picker — search, languages, recent picks. -->
        <button
          type="button"
          aria-pressed={readingText === "translation"}
          onclick={readingText === "translation" ? onPickTranslation : onReadTranslation}
          class={pill}
        >
          <span class="max-w-[11rem] truncate" dir="auto">
            {readingText === "translation" && readingFlowName ? readingFlowName : copy.shell.readingTranslation}
          </span>
        </button>
        <button
          type="button"
          aria-haspopup="dialog"
          aria-label={copy.shell.readingTranslationPick}
          title={copy.shell.readingTranslationPick}
          onclick={onPickTranslation}
          class="flex h-8 w-7 flex-none items-center justify-center rounded-sm text-foreground-secondary transition-colors hover:text-foreground"
        >
          <ChevronDownIcon class="size-3.5" />
        </button>
      {:else}
        <button type="button" aria-pressed={false} onclick={() => onChangeMode("reading")} class={pill}>
          <Icon name="continuous" size={13} />
          <span>{copy.shell.reading}</span>
        </button>
      {/if}
    </div>
  {/if}
</div>
