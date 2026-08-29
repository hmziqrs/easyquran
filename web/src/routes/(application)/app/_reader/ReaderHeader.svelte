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

  // Reader CHROME only (the reading column stays neutral): the header band takes
  // the hue-slot soft fill cycling by surah (§6), and the number chip renders the
  // legible pair on it — the landing tile grammar. Palette × mode resolve the pair.
  const HUE_SOFT = {
    1: "var(--hue-1-soft)",
    2: "var(--hue-2-soft)",
    3: "var(--hue-3-soft)",
    4: "var(--hue-4-soft)",
  } as const;
  const HUE_LEGIBLE = {
    1: "var(--hue-1-legible)",
    2: "var(--hue-2-legible)",
    3: "var(--hue-3-legible)",
    4: "var(--hue-4-legible)",
  } as const;
  type Hue = keyof typeof HUE_SOFT;

  function hueFor(surahNum: number): Hue {
    // SAFETY: ((surahNum-1) % 4) is 0–3 for any positive integer, so +1 is exactly 1–4.
    return (((surahNum - 1) % 4) + 1) as Hue;
  }
  const hue = $derived(hueFor(initial.surah.num));
</script>

<div
  class="flex min-h-[229px] flex-wrap items-start justify-between gap-6 border-b border-border px-5 pb-[26px] pt-[30px] sm:min-h-0 sm:px-9"
  style:background={HUE_SOFT[hue]}
>
  <div class="flex items-start gap-4">
    <div
      aria-hidden="true"
      class="flex h-16 w-16 flex-none items-center justify-center rounded-sm font-arabic text-lg font-bold"
      style:color={HUE_LEGIBLE[hue]}
    >
      {badge}
    </div>
    <div class="flex min-w-0 flex-col gap-1.5">
      <span class="text-xs font-semibold uppercase tracking-[0.1em] text-muted-foreground">
        {copy.shell.surahPage(initial.surah.num, visibleLocalPage, initial.pageCount)}
      </span>
      <div class="flex flex-wrap items-baseline gap-x-3.5 gap-y-1">
        <h1 class="text-[32px] font-semibold tracking-[-0.025em]">
          {initial.surah.num}. {initial.surah.name}
        </h1>
        <span dir="rtl" lang="ar" class="font-arabic text-[30px] leading-none text-foreground-secondary">
          {initial.surah.arabic}
        </span>
      </div>
      <span class="text-sm text-muted-foreground">{surahMeta(initial.surah)}</span>
    </div>
  </div>

  {#if clientMounted}
    <div class="flex flex-wrap items-center justify-end gap-2">
      <div
        class="flex items-center gap-0.5 rounded-md bg-background-subtle p-1"
        role="group"
        aria-label={copy.shell.arabicTextSizeLabel}
      >
        <button
          type="button"
          onclick={onSmaller}
          aria-label={copy.shell.smallerArabicTextLabel}
          class="flex h-[26px] w-7 items-center justify-center rounded-pill text-[13px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
        >
          A&minus;
        </button>
        <button
          type="button"
          onclick={onBigger}
          aria-label={copy.shell.largerArabicTextLabel}
          class="flex h-[26px] w-7 items-center justify-center rounded-pill text-[15px] text-foreground-secondary transition-colors hover:bg-surface-hover hover:text-foreground"
        >
          A+
        </button>
      </div>

      <div
        class="flex items-center gap-0.5 rounded-md bg-background-subtle p-1"
        role="group"
        aria-label={copy.shell.readingModeLabel}
      >
        <button
          type="button"
          aria-pressed={reader.isVerseMode}
          onclick={() => onChangeMode("verse")}
          class="flex h-[26px] items-center gap-1.5 rounded-pill px-2.5 text-[13px] font-medium transition-colors text-muted-foreground hover:bg-surface-hover hover:text-foreground aria-pressed:bg-primary aria-pressed:text-primary-foreground aria-pressed:hover:bg-primary aria-pressed:hover:text-primary-foreground"
        >
          <Icon name="rows" size={13} />
          <span class="hidden sm:inline">{copy.shell.ayahByAyah}</span>
          <span class="sm:hidden">{copy.shell.ayahs}</span>
        </button>
        <button
          type="button"
          aria-pressed={reader.isReadingMode}
          onclick={() => onChangeMode("reading")}
          class="flex h-[26px] items-center gap-1.5 rounded-pill px-2.5 text-[13px] font-medium transition-colors text-muted-foreground hover:bg-surface-hover hover:text-foreground aria-pressed:bg-primary aria-pressed:text-primary-foreground aria-pressed:hover:bg-primary aria-pressed:hover:text-primary-foreground"
        >
          <Icon name="continuous" size={13} />
          <span>{copy.shell.reading}</span>
        </button>
      </div>
    </div>
  {/if}
</div>
