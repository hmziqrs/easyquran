import { resolveManifest } from "./manifest";
import { quranWorker } from "./worker-client";
import { quran } from "$lib/stores/quran.svelte";
import { loadQuranCatalog } from "$lib/data/quran-metadata-client";

export function bootOfflineEngine(): () => void {
  quran.status = "resolving";

  const detachStatus = quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));
  const detachProgress = quranWorker.onProgress((p) => quran.setDownload(p));

  try {
    void navigator.storage?.persist?.();
  } catch {}

  void (async () => {
    try {
      const [manifest, catalog] = await Promise.all([resolveManifest(), loadQuranCatalog()]);
      quran.source = manifest.source;
      await quranWorker.start(manifest, catalog.coordinates);
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
