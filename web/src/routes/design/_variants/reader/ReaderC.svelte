<script lang="ts">
  import { toArabicDigits, surahMeta } from "$lib/data/quran";
  import type { Surah } from "$lib/data/quran";
  import { displayVerses } from "../verses";
  import { headerText } from "$lib/quran/view/presentation";

  let { surah }: { surah: Surah } = $props();

  const verses = $derived(displayVerses(surah));
  const opener = $derived(headerText(surah.normalization));

  let size = $state(28);
  const clamp = (n: number) => Math.max(18, Math.min(48, n));
</script>

<div class="mx-auto w-full max-w-[900px] px-5 py-10">
  <div class="mb-5 flex items-center justify-between gap-4">
    <span class="font-mono text-[11px] uppercase tracking-[0.12em] text-fg-4">
      Surah {surah.num} · {surahMeta(surah)}
    </span>
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
  </div>

  <div class="rounded-[18px] border border-line-3 bg-bg-1 p-2 shadow-[0_24px_60px_-34px_rgba(0,0,0,0.6)]">
    <div class="rounded-[12px] border border-accent-line">
      <div class="relative border-b border-accent-line px-6 py-5 text-center">
        <div
          aria-hidden="true"
          class="pointer-events-none absolute inset-0 rounded-t-[11px] opacity-60"
          style="background: linear-gradient(to bottom, var(--accent-soft), transparent)"
        ></div>
        <div class="relative flex flex-col items-center gap-1.5">
          <span dir="rtl" class="font-arabic text-[30px] leading-none text-fg">{surah.arabic}</span>
          <span class="text-[12.5px] tracking-[0.08em] text-fg-3">
            {surah.name} · {surah.ayahCount} ayahs
          </span>
        </div>
      </div>

      {#if opener}
        <p dir="rtl" class="border-b border-line px-6 py-5 text-center font-arabic text-[22px] text-fg-2">
          {opener}
        </p>
      {/if}

      <div
        dir="rtl"
        class="reading-text px-7 py-9 text-fg sm:px-11"
        style={`font-size:${size}px`}
      >{#each verses as text, i (i)}<span>{text}</span><span class="ayah-marker">{toArabicDigits(i + 1)}</span> {/each}</div>

      <div class="flex items-center justify-between border-t border-accent-line px-6 py-3.5">
        <span class="font-arabic text-[13px] text-fg-4">{toArabicDigits(surah.num)}</span>
        <span class="text-[11px] uppercase tracking-[0.14em] text-fg-4">Uthmani</span>
        <span class="font-arabic text-[13px] text-fg-4">{toArabicDigits(surah.ayahCount)}</span>
      </div>
    </div>
  </div>
</div>
