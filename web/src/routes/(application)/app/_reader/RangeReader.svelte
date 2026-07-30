<!--
  RangeReader — renders the ayahs of an open juz or page range (selected from the
  sidebar), grouped by surah. The ayahs come from the sqlite-wasm Worker
  (readRange over the cached Uthmani DB); a range is not prerendered, so this
  view shows a loading state until the offline engine is ready. Ayah text is
  rendered verbatim from the source (a surah's basmala is inline in its first
  ayah, so no separate header is added here).
-->
<script lang="ts">
  import { reader } from "$lib/stores/reader.svelte";
  import { surahByNum, surahPath } from "$lib/data/quran";
  import { goto } from "$app/navigation";
  import VerseRow from "./VerseRow.svelte";
  import type { Ayah } from "$lib/data/quran-types";

  interface Group {
    num: number;
    name: string;
    arabic: string;
    ayahs: Ayah[];
  }

  // Ayahs arrive in ascending global order; split into surah groups.
  const groups = $derived.by<Group[]>(() => {
    const out: Group[] = [];
    for (const a of reader.rangeAyahs) {
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

  const label = $derived(reader.rangeView?.label ?? "");

  function back(): void {
    reader.closeRange();
  }
  /** Open the full surah (leaves the range view). */
  function openSurah(num: number): void {
    reader.closeRange();
    void goto(surahPath(num));
  }
</script>

<div class="flex flex-col gap-4">
  <!-- range header: back + label -->
  <div class="flex items-center justify-between gap-3">
    <button
      type="button"
      onclick={back}
      class="flex items-center gap-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
    >
      <span aria-hidden="true">←</span>
      Back to surah
    </button>
    <span class="text-sm font-medium text-accent">{label}</span>
  </div>

  {#if reader.rangeLoading}
    <div
      class="rounded-2xl border border-line bg-bg-1 px-5 py-16 text-center text-sm text-fg-3 sm:px-9"
    >
      Loading {label}…
    </div>
  {:else if reader.rangeError}
    <div
      class="rounded-2xl border border-line bg-bg-1 px-5 py-16 text-center text-sm text-fg-3 sm:px-9"
    >
      Couldn’t load {label} from offline data. Open a surah first so the Quran
      database caches, then try again.
    </div>
  {:else}
    {#each groups as g (g.num)}
      <div class="overflow-hidden rounded-2xl border border-line bg-bg-1">
        <!-- surah group header -->
        <div
          class="flex items-center justify-between gap-3 border-b border-line px-5 py-3 sm:px-9"
        >
          <span class="text-sm font-semibold text-fg">{g.num}. {g.name}</span>
          <button
            type="button"
            onclick={() => openSurah(g.num)}
            class="text-[12.5px] text-accent transition-colors hover:brightness-110"
          >
            <span dir="rtl" class="font-arabic text-base">{g.arabic}</span>
            <span class="ml-2">Full surah →</span>
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
  {/if}
</div>
