<script lang="ts">
  import { surahMeta, type SurahLocalPageData } from "$lib/data/quran";
  import { Icon } from "$lib/components/icon";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";

  let {
    initial,
    visibleLocalPage,
    clientMounted,
    onChangeMode,
    onSmaller,
    onBigger,
  }: {
    initial: SurahLocalPageData;
    visibleLocalPage: number;
    clientMounted: boolean;
    onChangeMode: (mode: ReaderMode) => void;
    onSmaller: () => void;
    onBigger: () => void;
  } = $props();

  const copy = getReaderUiCopy();
  const badge = $derived(String(initial.surah.num).padStart(3, "0"));
</script>

<div
  class="flex min-h-[229px] flex-wrap items-start justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:min-h-0 sm:px-9"
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
        {copy.shell.surahPage(initial.surah.num, visibleLocalPage, initial.pageCount)}
      </span>
      <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
        <h1 class="text-[32px] font-semibold tracking-[-0.025em]">
          {initial.surah.num}. {initial.surah.name}
        </h1>
        <span dir="rtl" lang="ar" class="font-arabic text-[30px] leading-none text-fg-2">
          {initial.surah.arabic}
        </span>
      </div>
      <span class="text-sm text-fg-3">{surahMeta(initial.surah)}</span>
    </div>
  </div>

  {#if clientMounted}
    <div class="flex flex-wrap items-center justify-end gap-2">
      <div
        class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
        role="group"
        aria-label={copy.shell.arabicTextSizeLabel}
      >
        <button
          type="button"
          onclick={onSmaller}
          aria-label={copy.shell.smallerArabicTextLabel}
          class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
        >
          A&minus;
        </button>
        <button
          type="button"
          onclick={onBigger}
          aria-label={copy.shell.largerArabicTextLabel}
          class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
        >
          A+
        </button>
      </div>

      <div
        class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
        role="group"
        aria-label={copy.shell.readingModeLabel}
      >
        <button
          type="button"
          aria-pressed={reader.isVerseMode}
          onclick={() => onChangeMode("verse")}
          class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors aria-pressed:bg-bg-3 aria-pressed:text-fg text-fg-3 hover:text-fg"
        >
          <Icon name="rows" size={13} />
          <span class="hidden sm:inline">{copy.shell.ayahByAyah}</span>
          <span class="sm:hidden">{copy.shell.ayahs}</span>
        </button>
        <button
          type="button"
          aria-pressed={reader.isReadingMode}
          onclick={() => onChangeMode("reading")}
          class="flex h-[26px] items-center gap-1.5 rounded-md px-2.5 text-[13px] font-medium transition-colors aria-pressed:bg-bg-3 aria-pressed:text-fg text-fg-3 hover:text-fg"
        >
          <Icon name="continuous" size={13} />
          <span>{copy.shell.reading}</span>
        </button>
      </div>
    </div>
  {/if}
</div>
