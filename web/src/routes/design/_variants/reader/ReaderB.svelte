<script lang="ts">
  import { Icon } from "$lib/components";
  import { toArabicDigits, surahMeta } from "$lib/data/quran";
  import type { Surah } from "$lib/data/quran";
  import { cn } from "$lib/utils";
  import { displayVerses } from "../verses";
  import { headerText } from "$lib/quran/view/presentation";

  let { surah }: { surah: Surah } = $props();

  const verses = $derived(displayVerses(surah));
  const opener = $derived(headerText(surah.normalization));

  let size = $state(27);
  let active = $state(1);
  const clamp = (n: number) => Math.max(18, Math.min(48, n));

  function jump(n: number) {
    active = n;
    document.getElementById(`v-${n}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }
</script>

<div class="mx-auto grid w-full max-w-[1240px] gap-10 px-5 py-10 lg:grid-cols-[17rem_1fr]">
  <aside class="flex flex-col gap-6 lg:sticky lg:top-20 lg:max-h-[calc(100vh-6rem)] lg:self-start lg:overflow-y-auto">
    <div class="flex flex-col gap-2">
      <span class="eyebrow text-accent">Surah {surah.num}</span>
      <div class="flex items-baseline justify-between gap-3">
        <h1 class="text-[24px] tracking-[-0.02em]">{surah.name}</h1>
        <span dir="rtl" class="font-arabic text-[22px] leading-none text-fg-2">{surah.arabic}</span>
      </div>
      <p class="text-[13px] text-fg-4">{surahMeta(surah)}</p>
    </div>

    <div class="flex items-center gap-2">
      <div class="flex items-center gap-0.5 rounded-[9px] bg-bg-2 p-1" role="group" aria-label="Arabic text size">
        <button
          type="button"
          aria-label="Smaller Arabic text"
          onclick={() => (size = clamp(size - 2))}
          class="flex h-[26px] w-7 items-center justify-center rounded-md text-[13px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg">A&minus;</button
        >
        <button
          type="button"
          aria-label="Larger Arabic text"
          onclick={() => (size = clamp(size + 2))}
          class="flex h-[26px] w-7 items-center justify-center rounded-md text-[15px] text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg">A+</button
        >
      </div>
      <button
        type="button"
        class="flex h-[34px] flex-1 items-center justify-center gap-2 rounded-[9px] bg-accent-soft text-[13px] text-accent transition-[filter] hover:brightness-105"
      >
        <Icon name="play" size={14} /> Listen
      </button>
    </div>

    <div class="flex flex-col gap-2">
      <span class="font-mono text-[11px] uppercase tracking-[0.12em] text-fg-4">Jump to ayah</span>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(2rem,1fr))] gap-1">
        {#each verses as _, i (i)}
          <button
            type="button"
            onclick={() => jump(i + 1)}
            aria-label={`Go to ayah ${i + 1}`}
            class={cn(
              "flex h-8 items-center justify-center rounded-md font-mono text-[11px] transition-colors",
              active === i + 1
                ? "bg-accent text-accent-fg"
                : "bg-bg-2 text-fg-3 hover:bg-bg-3 hover:text-fg",
            )}
          >
            {i + 1}
          </button>
        {/each}
      </div>
    </div>
  </aside>

  <div class="min-w-0">
    {#if opener}
      <p dir="rtl" class="mb-8 border-b border-line pb-8 text-center font-arabic text-[22px] text-fg-3">
        {opener}
      </p>
    {/if}

    {#each verses as text, i (i)}
      <div
        id={`v-${i + 1}`}
        class={cn(
          "grid scroll-mt-24 grid-cols-[3rem_1fr] items-start gap-x-5 border-b border-line py-6 transition-colors",
          active === i + 1 && "bg-accent-soft/40",
        )}
      >
        <div class="flex flex-col items-center gap-1 pt-1.5">
          <span
            class="flex size-8 items-center justify-center rounded-full border border-accent-line font-arabic text-[13px] text-accent"
          >
            {toArabicDigits(i + 1)}
          </span>
          <span class="font-mono text-[10px] text-fg-4">{surah.num}:{i + 1}</span>
        </div>

        <p dir="rtl" class="font-arabic leading-[2.2] text-fg" style={`font-size:${size}px`}>
          {text}
        </p>
      </div>
    {/each}
  </div>
</div>
