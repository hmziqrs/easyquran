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
</div>
