<script lang="ts">
  import type { TranslationCatalogueEntry } from "#lib/data/quran-types.js";
  import type { SearchCopy } from "#lib/i18n/search-copy.js";
  import { translationAuthorKey } from "#lib/quran/translation-groups.js";
  import { quranWorker } from "#lib/quran/worker-client.js";
  import { searchSelection } from "#lib/stores/search-selection.svelte.js";
  import { translationRecents } from "#lib/stores/translation-recents.svelte.js";
  import { formatBytes } from "#lib/utils.js";

  let {
    entry,
    copy,
    selected,
    cached,
    progress,
  }: {
    entry: TranslationCatalogueEntry;
    copy: SearchCopy;
    selected: boolean;
    cached: boolean;
    progress: { loaded: number; total: number } | null;
  } = $props();

  const rtl = $derived(entry.direction === "rtl");
  const pct = $derived(
    progress !== null && progress.total > 0
      ? Math.round((progress.loaded / progress.total) * 100)
      : null,
  );

  function download(): void {
    if (cached || rtl) return;
    void quranWorker.ensureTranslation(entry.id).catch(() => {});
  }

  function toggle(): void {
    searchSelection.toggle(entry.id);
    // A search-page pick is an explicit selection too (Feature 1.1): it
    // records into the MRU and the author's variant memory; unchecking is not.
    if (!selected) translationRecents.record(entry.id, Date.now(), translationAuthorKey(entry));
  }
</script>

<div
  class="flex flex-col gap-1.5 rounded-sm px-3 py-2 text-[12.5px] transition-colors hover:bg-surface-hover"
>
  <div class="flex items-center gap-2">
    <input
      id={`search-t-${entry.id}`}
      type="checkbox"
      checked={selected}
      disabled={rtl}
      onchange={toggle}
      class="h-4 w-4 flex-none accent-primary"
    />
    <label for={`search-t-${entry.id}`} class="min-w-0 flex-1 truncate text-foreground-secondary">
      {entry.name}
      {#if entry.translator !== null}
        <span class="text-muted"> · {entry.translator}</span>
      {/if}
    </label>

    {#if cached}
      <span class="flex-none rounded-pill bg-surface-hover px-1.5 py-0.5 text-[10px] text-muted">
        {copy.cached} · {formatBytes(entry.sizeBytes)}
      </span>
    {:else if rtl}
      <span class="flex-none text-[11px] text-muted">{copy.rtlDisabled}</span>
    {:else}
      <button
        type="button"
        onclick={download}
        disabled={pct !== null}
        class="flex-none rounded-pill border border-border-strong px-2.5 py-1 text-[11px] text-foreground-secondary transition-colors hover:border-border hover:text-foreground disabled:opacity-60"
      >
        {#if pct !== null}
          {copy.downloading}
        {:else}
          {copy.download}
        {/if}
      </button>
    {/if}
  </div>

  {#if pct !== null && !cached}
    <div class="ms-6 h-1 w-full max-w-[280px] overflow-hidden rounded-pill bg-surface-hover">
      <div class="h-full rounded-pill bg-primary transition-[width]" style:width={`${pct}%`}></div>
    </div>
  {/if}
</div>
