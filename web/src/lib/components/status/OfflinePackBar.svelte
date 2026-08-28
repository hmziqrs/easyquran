<script lang="ts">
  import { offline } from "$lib/offline/offline-store.svelte";
  import type { OfflinePackCopy } from "$lib/components/status/offline-pack-copy";

  let { copy }: { copy: OfflinePackCopy } = $props();

  const working = $derived(offline.status === "downloading" || offline.status === "staging");
  const pct = $derived(working ? offline.pct : 0);
  const milestone = $derived.by(() => {
    if (!working) return "";
    if (pct >= 100) return copy.barReady;
    const bucket = Math.min(100, Math.floor(pct / 25) * 25);
    return bucket === 0 ? copy.barPreparing : `${bucket}%`;
  });
</script>

{#if working}
  <div
    class="pointer-events-none fixed inset-x-0 top-0 z-[80] flex flex-col items-center gap-1.5 px-3 pt-3"
  >
    <div
      class="pointer-events-auto flex w-full max-w-sm items-center gap-2 rounded-pill border border-border bg-surface px-3.5 py-2 text-caption text-foreground shadow-md"
    >
      <span class="inline-block size-1.5 shrink-0 animate-pulse rounded-full bg-primary"></span>
      <span class="truncate">{copy.barPreparing}</span>
      <span class="ms-auto tabular-nums opacity-70" aria-hidden="true">{pct}%</span>
    </div>
    <div class="h-1 w-full max-w-sm overflow-hidden rounded-pill border border-border bg-background-subtle">
      <div
        class="h-full rounded-pill bg-primary transition-[width] duration-150 ease-out"
        style={`width:${pct}%`}
      ></div>
    </div>
    <span class="sr-only" aria-live="polite">{milestone}</span>
  </div>
{/if}
