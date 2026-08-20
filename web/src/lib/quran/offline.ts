import { loadQuranData } from "$lib/data/quran-data-client";
import { quran } from "$lib/stores/quran.svelte";

import { bakedTranslationCatalogue } from "./catalogue";
import { bakedManifest } from "./manifest";
import { quranWorker } from "./worker-client";

export function bootOfflineEngine(): () => void {
  quran.status = "resolving";
  let active = true;

  const detachStatus = quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));
  const detachProgress = quranWorker.onProgress((p) => quran.setDownload(p));

  const persistenceRequest = navigator.storage?.persist?.();
  if (persistenceRequest !== undefined) void persistenceRequest.catch(() => false);

  void (async () => {
    try {
      const quranData = await loadQuranData();
      if (!active) return;
      const manifest = bakedManifest();
      const catalogue = bakedTranslationCatalogue();
      await quranWorker.start(manifest, quranData.coordinates, catalogue);
    } catch (e) {
      if (!active) return;
      quran.status = "error";
      quran.error = e instanceof Error ? e.message : String(e);
    }
  })();

  return () => {
    active = false;
    detachStatus();
    detachProgress();
    quranWorker.dispose();
  };
}
