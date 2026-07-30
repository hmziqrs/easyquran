/* ════════════════════════════════════════════════════════════════════════
   offline.ts — boot the offline Quran engine (called once, after mount).

   Resolves the manifest (live API when up, else baked), wires worker lifecycle
   events onto the quran status store, and starts the sqlite-wasm Worker. Best-
   effort `navigator.storage.persist()`. Every step degrades silently — the
   reader always works from prerendered data; this only adds offline + search.
   ════════════════════════════════════════════════════════════════════════ */

import { resolveManifest } from "./manifest";
import { quranWorker } from "./worker-client";
import { quran } from "$lib/stores/quran.svelte";

export async function bootOfflineEngine(): Promise<void> {
  quran.status = "resolving";
  const manifest = await resolveManifest();
  quran.source = manifest.source;

  quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));

  // Best-effort: persists the OPFS cache across eviction where the origin has
  // enough engagement. Denied silently otherwise; eviction = redownload.
  try {
    void navigator.storage?.persist?.();
  } catch {
    /* non-fatal */
  }

  try {
    await quranWorker.start(manifest);
  } catch (e) {
    quran.status = "error";
    quran.error = e instanceof Error ? e.message : String(e);
  }
}
