<script lang="ts">
  import { surahMeta, type SurahLocalPageData } from "#lib/data/quran.js";
  import { ReadingTextPicker } from "#lib/components/translation/index.js";
  import { reader } from "#lib/stores/reader.svelte.js";
  import type { ReadingText } from "#lib/stores/reading-text.svelte.js";

  let {
    initial,
    clientMounted,
    readingText,
    readingFlowName,
    onReadArabic,
    onReadTranslation,
    onPickTranslation,
  }: {
    initial: SurahLocalPageData;
    clientMounted: boolean;
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
</script>

<!-- Strip header (user pick, /design/mix head=e): one compact row — number, name, Arabic
     name, meta — so the text starts sooner. No hue band; A−/A+ live in the sticky bar. -->
<div class="flex flex-wrap items-center gap-x-4 gap-y-3 border-b border-reader-divider py-4">
  <span
    aria-hidden="true"
    class="flex size-9 flex-none items-center justify-center rounded-pill bg-primary text-[13px] font-medium text-primary-foreground"
  >
    {initial.surah.num}
  </span>
  <div class="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-3 gap-y-0.5">
    <h1 class="text-[21px] font-medium tracking-[-0.015em] text-foreground">
      <span class="sr-only">{initial.surah.num}. </span>{initial.surah.name}
    </h1>
    <span dir="rtl" lang="ar" class="font-arabic text-[24px] leading-none text-foreground">
      {initial.surah.arabic}
    </span>
    <span class="text-[13.5px] text-foreground-secondary">{surahMeta(initial.surah)}</span>
  </div>

  {#if clientMounted && reader.isReadingMode}
    <!-- Ayah-by-Ayah / Reading lives in the sticky bar (ReaderModeToggle), reachable from
         anywhere. Here Reading only picks its text (quran.com's model): Arabic or one
         translation — one tap; tapping Translation again (or ▾) opens the full picker. -->
    <ReadingTextPicker
      class="sm:ms-auto sm:w-auto"
      flowing={readingText}
      translationName={readingText === "translation" ? readingFlowName : null}
      onArabic={onReadArabic}
      onTranslation={readingText === "translation" ? onPickTranslation : onReadTranslation}
      onOpen={onPickTranslation}
    />
  {/if}
</div>
