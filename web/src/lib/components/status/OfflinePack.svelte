<script lang="ts">
  import { offline } from "$lib/offline/offline-store.svelte";
  import { cn } from "$lib/utils";

  const pill = "rounded-md border px-3 py-1.5 text-xs transition-colors duration-150";

  const working = $derived(offline.status === "downloading" || offline.status === "staging");
  const pct = $derived(working ? offline.pct : 0);
  const savedAt = $derived(offline.activePack?.savedAt ?? null);

  function formatBytes(n: number | null): string {
    if (n == null || n <= 0) return "—";
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function toggle() {
    if (offline.activePack) await offline.disable();
    else await offline.enable();
  }

  let label = $derived(
    offline.busy && !working
      ? "Working…"
      : offline.status === "error"
        ? "Retry"
        : offline.activePack
          ? "Remove offline"
          : "Download for offline",
  );
</script>

<section class="grid gap-1.5">
  <div class="flex items-center justify-between gap-2">
    <span class="text-xs text-fg-3">Offline</span>
    <span class="text-right text-[11px] leading-tight text-fg-3">{offline.statusText}</span>
  </div>
  {#if offline.activePack}
    <div class="text-[11px] text-fg-4">
      {offline.activePack.entries} routes · {formatBytes(offline.activePack.bytes)}{#if savedAt}
        · saved {new Date(savedAt).toLocaleDateString()}{/if}
    </div>
  {/if}
  {#if offline.quota != null}
    <div class="text-[11px] text-fg-4">
      Storage: {formatBytes(offline.usage)} of {formatBytes(offline.quota)}
    </div>
  {/if}
  <button
    type="button"
    disabled={offline.busy}
    onclick={toggle}
    aria-pressed={!!offline.activePack}
    class={cn(
      pill,
      offline.activePack
        ? "border-line-2 text-fg-2 hover:text-fg"
        : "border-accent bg-accent-soft text-fg hover:opacity-90",
      offline.busy && "cursor-not-allowed opacity-50",
    )}
  >
    {label}
  </button>
</section>

{#if working}
  <div
    class="pointer-events-none fixed inset-x-0 top-0 z-[80] flex flex-col items-center gap-1.5 px-3 pt-3"
    role="status"
    aria-live="polite"
  >
    <div
      class="pointer-events-auto flex w-full max-w-sm items-center gap-2 rounded-full bg-bg-1 px-3.5 py-2 text-xs text-fg shadow-lg ring-1 ring-black/10"
    >
      <span class="inline-block size-1.5 shrink-0 animate-pulse rounded-full bg-accent"></span>
      <span class="truncate">Preparing offline pack</span>
      <span class="ml-auto tabular-nums opacity-70">{pct}%</span>
    </div>
    <div class="h-1 w-full max-w-sm overflow-hidden rounded-full bg-bg-2 ring-1 ring-black/10">
      <div
        class="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
        style={`width:${pct}%`}
      ></div>
    </div>
  </div>
{/if}
