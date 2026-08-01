/*
   offline.ts — boot the offline Quran engine (called once, after mount).

   Resolves the manifest (live API when up, else baked), wires worker lifecycle
   events onto the quran status store, and starts the sqlite-wasm Worker. Best-
   effort `navigator.storage.persist()`. Every step degrades silently — the
   reader always works from prerendered data; this only adds offline + search.

   Returns a teardown that detaches the two store-forwarding listeners so the
   quran store stops receiving worker events once the boot service is disposed
   (the worker itself is a singleton that lives for the page session). The boot
   is fire-and-forget: the returned teardown is synchronous and safe to call
   before the async manifest/worker sequence settles.
*/

import { resolveManifest } from "./manifest";
import { quranWorker } from "./worker-client";
import { quran } from "$lib/stores/quran.svelte";

export function bootOfflineEngine(): () => void {
  quran.status = "resolving";

  // Forward worker lifecycle + download progress onto the reactive quran store
  // so the UI can reflect them. Captured here so teardown can detach them —
  // previously these were registered with no way to remove them.
  const detachStatus = quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));
  const detachProgress = quranWorker.onProgress((p) => quran.setDownload(p));

  // Best-effort: persists the OPFS cache across eviction where the origin has
  // enough engagement. Denied silently otherwise; eviction = redownload.
  try {
    void navigator.storage?.persist?.();
  } catch {
  }

  void (async () => {
    try {
      const manifest = await resolveManifest();
      quran.source = manifest.source;
      await quranWorker.start(manifest);
    } catch (e) {
      quran.status = "error";
      quran.error = e instanceof Error ? e.message : String(e);
    }
  })();

  return () => {
    detachStatus();
    detachProgress();
  };
}
