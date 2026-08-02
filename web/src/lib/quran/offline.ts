import { resolveManifest } from "./manifest";
import { quranWorker } from "./worker-client";
import { quran } from "$lib/stores/quran.svelte";
import { loadQuranData } from "$lib/data/quran-data-client";

export function bootOfflineEngine(): () => void {
  quran.status = "resolving";
  let active = true;

  const detachStatus = quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));
  const detachProgress = quranWorker.onProgress((p) => quran.setDownload(p));

  const persistenceRequest = navigator.storage?.persist?.();
  if (persistenceRequest) void persistenceRequest.catch(() => false);

  void (async () => {
    try {
      const [manifest, quranData] = await Promise.all([resolveManifest(), loadQuranData()]);
      if (!active) return;
      quran.source = manifest.source;
      await quranWorker.start(manifest, quranData.coordinates);
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
