<!--
  Results — the search-results list shown in place of the reader when
  reader.hasQuery. Backed by quranSearch: the offline Worker corpus when ready
  (real verse text), the live API when up, or a surah name/number fallback while
  the corpus loads. Each card shows the verse ref + Uthmani text (or just the
  ref for name matches); clicking opens it in the reader.
-->
<script lang="ts">
  import { goto } from "$app/navigation";
  import { reader } from "$lib/stores/reader.svelte";
  import { surahByNum, surahPath } from "$lib/data/quran";
  import { quranSearch } from "$lib/quran/search";
  import type { SearchResponse } from "$lib/quran/search/normalize";

  let result = $state.raw<SearchResponse | null>(null);
  let loading = $state(false);

  // Re-run search whenever the query changes (debounced so fast typing doesn't
  // spam the Worker).
  $effect(() => {
    const q = reader.query; // tracked dependency
    let cancelled = false;
    loading = true;
    const timer: ReturnType<typeof setTimeout> = setTimeout(() => {
      void (async () => {
        const trimmed = q.trim();
        const r = trimmed ? await quranSearch(trimmed) : null;
        if (!cancelled) {
          result = r;
          loading = false;
        }
      })();
    }, 140);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  const label = $derived(
    loading || !result
      ? "Searching…"
      : result.total === 0
        ? `No verses match “${result.query.trim()}”.`
        : result.source === "names"
          ? `${result.total} surah${result.total === 1 ? "" : "s"} matching “${result.query.trim()}”`
          : `${result.total} verse${result.total === 1 ? "" : "s"} matching “${result.query.trim()}”`,
  );

  function open(r: { surah: number; ayah: number }): void {
    reader.openVerse(r.surah, r.ayah);
    void goto(surahPath(r.surah, r.ayah));
  }
</script>

<div class="flex flex-col gap-3">
  <div class="text-sm text-fg-2">{label}</div>
  {#if result}
    {#each result.results as r (r.key)}
      <button
        type="button"
        onclick={() => open(r)}
        class="flex flex-col gap-2.5 rounded-[13px] border border-line bg-bg-1 px-6 py-5 text-left transition-colors hover:border-accent"
      >
        <span class="text-xs font-semibold uppercase tracking-[0.08em] text-accent">
          {surahByNum(r.surah).name} {r.surah}:{r.ayah}
        </span>
        {#if r.text}
          <span dir="rtl" class="font-arabic text-[26px] leading-[2] text-fg">{r.text}</span>
        {:else}
          <span class="text-sm text-fg-3">Open surah →</span>
        {/if}
      </button>
    {/each}
  {/if}
</div>
