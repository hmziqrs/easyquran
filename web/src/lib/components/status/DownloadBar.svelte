<script lang="ts">
  import { sumBy } from "es-toolkit";
  import { quran } from "$lib/stores/quran.svelte";
  import { QURAN } from "$lib/config/site";
  import { isArabicSourceId, QuranScript } from "$lib/data/quran-types";
  import { sourceProfile } from "$lib/quran/view/source-profiles";

  const SCRIPT_LABELS: Readonly<Record<QuranScript, string>> = {
    [QuranScript.Uthmani]: "Uthmani",
    [QuranScript.SimpleClean]: "Simple-clean",
    [QuranScript.IndoPak]: "IndoPak",
    [QuranScript.Tajweed]: "Tajweed",
    [QuranScript.Translation]: "Translation",
  };

  const sizeOf = new Map(QURAN.scripts.map((s) => [s.id, s.sizeBytes]));
  const totalBytes = sumBy(QURAN.scripts, (s) => s.sizeBytes);

  function overallFraction(): number | null {
    const d = quran.download;
    if (!d) return null;
    if (!isArabicSourceId(d.script)) {
      return d.total > 0 ? Math.min(1, d.loaded / d.total) : null;
    }
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
  function downloadLabel(): string {
    const script = quran.download?.script;
    if (script === undefined) return "";
    if (isArabicSourceId(script)) return SCRIPT_LABELS[sourceProfile(script).script];
    return SCRIPT_LABELS[QuranScript.Translation];
  }

  const label = $derived(downloadLabel());
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
