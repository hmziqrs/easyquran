<!--
  Results — the search-results list shown in place of the reader when
  reader.hasQuery. Each card shows the verse ref as an accent eyebrow and the
  Arabic text; clicking opens the verse in the reader (reader.openVerse).
-->
<script lang="ts">
  import { reader } from "$lib/stores/reader.svelte";
  import { searchVerses } from "$lib/data/quran";

  const results = $derived(searchVerses(reader.query));
  const label = $derived(
    results.length
      ? `${results.length} verse${results.length === 1 ? "" : "s"} matching “${reader.query.trim()}”`
      : `No verses match “${reader.query.trim()}” in this demo selection.`,
  );
</script>

<div class="flex flex-col gap-3">
  <div class="text-sm text-fg-2">{label}</div>
  {#each results as r (r.key)}
    <button
      type="button"
      onclick={() => reader.openVerse(r.num, r.ayah)}
      class="flex flex-col gap-2.5 rounded-[13px] border border-line bg-bg-1 px-6 py-5 text-left transition-colors hover:border-accent"
    >
      <span class="text-xs font-semibold uppercase tracking-[0.08em] text-accent">{r.ref}</span>
      <span dir="rtl" class="font-arabic text-[26px] leading-[2] text-fg">{r.text}</span>
    </button>
  {/each}
</div>
