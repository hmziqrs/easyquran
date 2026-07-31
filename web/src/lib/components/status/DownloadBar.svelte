<!--
  DownloadBar — a slim, top-of-viewport progress indicator shown while the
  offline engine downloads the two Arabic SQLite files into OPFS.

  Reads the `quran` status store (quran.download / quran.downloadPct). Progress
  is reported per-artifact by the worker (see quran/protocol.ts); here it is
  rolled into ONE 0..1 fraction across both files (they download sequentially in
  QURAN.scripts order) so the bar fills smoothly instead of resetting between
  the Uthmani and simple-clean downloads.

  Non-modal: pointer-events-none, so it never blocks the page. Auto-hides once
  the engine reaches `ready` (or `error`). No verse rendering depends on this —
  the reader paints from prerendered data regardless.
-->
<script lang="ts">
  import { quran } from "$lib/stores/quran.svelte";
  import { QURAN } from "$lib/config/site";

  /** Map each script → its immutable byte size (from the baked artifact spec). */
  const sizeOf = new Map(QURAN.scripts.map((s) => [s.id, s.sizeBytes]));
  /** Total bytes across both files (the denominator for overall progress). */
  const totalBytes = QURAN.scripts.reduce((sum, s) => sum + s.sizeBytes, 0);

  /** Overall 0..1 fraction across both sequential downloads, or null if idle. */
  function overallFraction(): number | null {
    const d = quran.download;
    if (!d) return null;
    let acc = 0;
    for (const s of QURAN.scripts) {
      if (s.id === d.script) {
        const sz = sizeOf.get(s.id) ?? d.total;
        return totalBytes > 0 ? Math.min(1, (acc + Math.min(d.loaded, sz)) / totalBytes) : null;
      }
      acc += sizeOf.get(s.id) ?? 0;
    }
    return null;
  }

  const visible = $derived(quran.download != null && quran.status !== "ready" && quran.status !== "error");
  const frac = $derived(overallFraction());
  const pct = $derived(frac == null ? 0 : Math.round(frac * 100));
  const label = $derived(
    quran.download?.script === "uthmani" ? "Uthmani" : quran.download?.script === "simple-clean" ? "Simple-clean" : "",
  );
</script>

{#if visible}
  <div class="pointer-events-none fixed inset-x-0 top-0 z-[80] flex flex-col items-center gap-1.5 px-3 pt-3" role="status" aria-live="polite">
    <div class="pointer-events-auto flex w-full max-w-sm items-center gap-2 rounded-full bg-bg-1 px-3.5 py-2 text-xs text-fg shadow-lg ring-1 ring-black/10">
      <span class="inline-block size-1.5 shrink-0 animate-pulse rounded-full bg-accent"></span>
      <span class="truncate">Preparing offline Quran{#if label}<span class="opacity-60"> · {label}</span>{/if}</span>
      <span class="ml-auto tabular-nums opacity-70">{pct}%</span>
    </div>
    <div class="h-1 w-full max-w-sm overflow-hidden rounded-full bg-bg-2 ring-1 ring-black/10">
      <div class="h-full rounded-full bg-accent transition-[width] duration-150 ease-out" style={`width:${pct}%`}></div>
    </div>
  </div>
{/if}
