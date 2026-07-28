<!--
  SurahReader — the main reading surface shown when there is no search query.
  Composes:
    · the optional "Continue reading" banner (reader.hasLastRead);
    · the surah Card — header (Surah {num} eyebrow, name + arabic + meta), an
      A−/A+ segmented control (reader.smaller/bigger) and a Play surah button
      (reader.playSurah);
    · the verse list, one VerseRow per ayah;
    · the prev/next footer (adjacentSurahs → reader.setCurrent).
-->
<script lang="ts">
  import { reader } from "$lib/stores/reader.svelte";
  import { adjacentSurahs, surahMeta, verseKey } from "$lib/data/quran";
  import { Button } from "$lib/components/ui/button";
  import { Icon } from "$lib/components/icon";
  import VerseRow from "./VerseRow.svelte";

  const surah = $derived(reader.surah);
  const adj = $derived(adjacentSurahs(reader.current));

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
    <!-- header -->
    <div
      class="flex flex-wrap items-end justify-between gap-6 border-b border-line px-5 pb-[26px] pt-[30px] sm:px-9"
    >
      <div class="flex flex-col gap-1.5">
        <span class="text-xs font-semibold uppercase tracking-[0.1em] text-accent">
          Surah {surah.num}
        </span>
        <div class="flex items-baseline gap-3.5">
          <h1 class="text-[32px] font-semibold tracking-[-0.025em]">{surah.name}</h1>
          <span dir="rtl" class="font-arabic text-[30px] text-fg-2">{surah.arabic}</span>
        </div>
        <span class="text-sm text-fg-3">{surahMeta(surah)}</span>
      </div>

      <div class="flex items-center gap-2">
        <!-- A− / A+ segmented font-size control -->
        <div
          class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1"
          role="group"
          aria-label="Arabic text size"
        >
          <button
            type="button"
            onclick={() => reader.smaller()}
            aria-label="Smaller Arabic text"
            title="Smaller Arabic"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A&minus;
          </button>
          <button
            type="button"
            onclick={() => reader.bigger()}
            aria-label="Larger Arabic text"
            title="Larger Arabic"
            class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
          >
            A+
          </button>
        </div>

        <Button variant="accent" size="md" onclick={() => reader.playSurah(reader.current)}>
          <Icon name="play" size={11} />
          Play surah
        </Button>
      </div>
    </div>

    <!-- verses -->
    <div class="flex flex-col">
      {#each surah.verses as text, i (i)}
        <VerseRow text={text} n={i + 1} vKey={verseKey(surah.num, i + 1)} />
      {/each}
    </div>

    <!-- prev / next -->
    <div class="flex items-center justify-between gap-4 px-5 py-[22px] sm:px-9">
      <button
        type="button"
        onclick={() => reader.setCurrent(adj.prev.num)}
        class="text-sm text-fg-2 transition-colors hover:text-fg"
      >
        ← {adj.prev.name}
      </button>
      <button
        type="button"
        onclick={() => reader.setCurrent(adj.next.num)}
        class="text-sm text-fg-2 transition-colors hover:text-fg"
      >
        {adj.next.name} →
      </button>
    </div>
  </div>

  <p class="px-1 text-[12.5px] leading-relaxed text-fg-3">
    This demo carries a selection of surahs and sample tafsir text. Recitation audio is simulated.
  </p>
</div>
