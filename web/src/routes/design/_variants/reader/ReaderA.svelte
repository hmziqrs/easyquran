<!--
  Reader A — "Focus".

  No card, no borders, no frame. One centred column at a comfortable measure,
  wide leading, and ayahs separated by space alone. All chrome collapses into a
  single floating bar that fades to near-transparent until you approach it, so
  in the steady state the only thing on screen is the text.

  Font size is local component state rather than the shared reader store — the
  gallery must not write the user's real reading preferences while they're
  clicking around comparing layouts.
-->
<script lang="ts">
  import { Icon } from "$lib/components";
  import { toArabicDigits, surahMeta } from "$lib/data/quran";
  import type { Surah } from "$lib/data/quran";
  import { displayVerses } from "../verses";
  import { headerText } from "$lib/quran/view/presentation";

  let { surah }: { surah: Surah } = $props();

  // Ayah 1 carries the basmala inline in the source text; the header renders it
  // separately, so it's stripped here to avoid showing it twice (see verses.ts).
  const verses = $derived(displayVerses(surah));
  const opener = $derived(headerText(surah.normalization));

  let size = $state(30);
  const clamp = (n: number) => Math.max(20, Math.min(56, n));
</script>

<div class="relative">
  <!-- masthead: barely there -->
  <header class="mx-auto w-full max-w-[46rem] px-6 pb-10 pt-16 text-center">
    <span class="eyebrow text-accent">Surah {surah.num}</span>
    <h1 class="mt-3 text-[28px] tracking-[-0.02em]">{surah.name}</h1>
    <p dir="rtl" class="mt-2 font-arabic text-[26px] leading-none text-fg-2">{surah.arabic}</p>
    <p class="mt-3 text-[13px] text-fg-4">{surahMeta(surah)}</p>
  </header>

  {#if opener}
    <p dir="rtl" class="mx-auto max-w-[46rem] px-6 pb-10 text-center font-arabic text-[22px] text-fg-3">
      {opener}
    </p>
  {/if}

  <!-- the text: space is the only separator -->
  <div class="mx-auto w-full max-w-[46rem] px-6 pb-40">
    {#each verses as text, i (i)}
      <p
        dir="rtl"
        class="py-7 font-arabic leading-[2.3] text-fg"
        style={`font-size:${size}px`}
      >
        {text}<span class="ayah-marker">{toArabicDigits(i + 1)}</span>
      </p>
    {/each}
  </div>

  <!-- the only chrome. Dims until hovered/focused so it never competes with
       the script; still fully reachable by keyboard (focus-within). -->
  <div
    class="fixed bottom-6 left-1/2 z-40 flex -translate-x-1/2 items-center gap-1 rounded-full border border-line-2 bg-bg-1/90 px-2 py-1.5 opacity-45 shadow-lg backdrop-blur transition-opacity duration-200 hover:opacity-100 focus-within:opacity-100"
  >
    <button
      type="button"
      aria-label="Smaller Arabic text"
      onclick={() => (size = clamp(size - 2))}
      class="flex size-8 items-center justify-center rounded-full text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
    >
      A&minus;
    </button>
    <button
      type="button"
      aria-label="Larger Arabic text"
      onclick={() => (size = clamp(size + 2))}
      class="flex size-8 items-center justify-center rounded-full text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
    >
      A+
    </button>
    <span class="mx-1 h-5 w-px bg-line-2"></span>
    <button
      type="button"
      aria-label="Search"
      class="flex size-8 items-center justify-center rounded-full text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
    >
      <Icon name="search" size={15} />
    </button>
    <button
      type="button"
      aria-label="Play recitation"
      class="flex size-8 items-center justify-center rounded-full text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg"
    >
      <Icon name="play" size={15} />
    </button>
    <span class="px-2 font-mono text-[11px] text-fg-4">{surah.ayahCount} ayahs</span>
  </div>
</div>
