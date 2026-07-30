<!--
  RangeReader — renders the ayahs of a juz or page, grouped by surah. Receives
  the ayahs prerendered in page.data (SSG reads the range from quran-uthmani.sqlite
  at build), so there is no loading state. Ayah text is verbatim from the source
  (a surah's basmala is inline in its first ayah, so no separate header is added).
-->
<script lang="ts">
  import { goto } from "$app/navigation";
  import { surahByNum, surahPath } from "$lib/data/quran";
  import VerseRow from "./VerseRow.svelte";
  import type { Ayah, RangePageData } from "$lib/data/quran-types";

  let { data }: { data: RangePageData } = $props();

  interface Group {
    num: number;
    name: string;
    arabic: string;
    ayahs: Ayah[];
  }

  // Ayahs arrive in ascending global order; split into surah groups.
  const groups = $derived.by<Group[]>(() => {
    const out: Group[] = [];
    for (const a of data.ayahs) {
      let g = out[out.length - 1];
      if (!g || g.num !== a.surah) {
        const s = surahByNum(a.surah);
        g = { num: a.surah, name: s.name, arabic: s.arabic, ayahs: [] };
        out.push(g);
      }
      g.ayahs.push(a);
    }
    return out;
  });

  /** Open the full surah (leaves the range view). */
  function openSurah(num: number): void {
    void goto(surahPath(num));
  }

  // Bounded prev/next pagination within the range's family (juz 1..30, page 1..604).
  const MAX = $derived(data.kind === "juz" ? 30 : 604);
  const kindLabel = $derived(data.kind === "juz" ? "Juz" : "Page");
  const prevHref = $derived(data.index > 1 ? `/app/${data.kind}/${data.index - 1}` : null);
  const nextHref = $derived(data.index < MAX ? `/app/${data.kind}/${data.index + 1}` : null);
</script>

<div class="flex flex-col gap-4">
  {#each groups as g (g.num)}
    <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
      <!-- surah group header -->
      <div class="flex items-center justify-between gap-3 border-b border-line px-5 py-3 sm:px-9">
        <span class="text-sm font-semibold text-fg">{g.num}. {g.name}</span>
        <button
          type="button"
          onclick={() => openSurah(g.num)}
          class="flex items-center gap-2 text-[12.5px] text-accent transition-colors hover:brightness-110"
        >
          <span dir="rtl" class="font-arabic text-base">{g.arabic}</span>
          <span>Full surah →</span>
        </button>
      </div>
      <!-- ayahs -->
      <div class="flex flex-col">
        {#each g.ayahs as a (a.key)}
          <VerseRow text={a.text} n={a.ayah} vKey={a.key} />
        {/each}
      </div>
    </div>
  {/each}

  <!-- prev / next within the family (juz 1..30, page 1..604) -->
  {#if prevHref || nextHref}
    <div
      class="flex items-center justify-between gap-4 rounded-2xl border border-line bg-bg-1 px-5 py-[22px] sm:px-9"
    >
      {#if prevHref}
        <a
          href={prevHref}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          <span aria-hidden="true">←</span> {kindLabel} {data.index - 1}
        </a>
      {:else}
        <span></span>
      {/if}
      {#if nextHref}
        <a
          href={nextHref}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {kindLabel} {data.index + 1} <span aria-hidden="true">→</span>
        </a>
      {/if}
    </div>
  {/if}
</div>
