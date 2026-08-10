import { QURAN } from "$lib/config/site";
import { bakedTranslationCatalogue } from "./catalogue";
import { ManifestSource, resolveManifest, type ResolvedManifest } from "./manifest";
import { quranWorker } from "./worker-client";
import { quran } from "$lib/stores/quran.svelte";
import { loadQuranData } from "$lib/data/quran-data-client";
import { catalogueStore } from "./catalogue-store.svelte";

function bakedManifest(): ResolvedManifest {
  return { scripts: QURAN.scripts, source: ManifestSource.Baked };
}

export function bootOfflineEngine(): () => void {
  quran.status = "resolving";
  let active = true;

  const detachStatus = quranWorker.onStatus((s, detail) => quran.setWorkerStatus(s, detail));
  const detachProgress = quranWorker.onProgress((p) => quran.setDownload(p));

  const persistenceRequest = navigator.storage?.persist?.();
  if (persistenceRequest) void persistenceRequest.catch(() => false);

  void (async () => {
    try {
      const quranData = await loadQuranData();
      if (!active) return;
      const manifest = bakedManifest();
      const catalogue = bakedTranslationCatalogue();
      quran.source = manifest.source;
      await quranWorker.start(manifest, quranData.coordinates, catalogue);
      if (!active) return;
      void refreshBootMetadata();
    } catch (e) {
      if (!active) return;
      quran.status = "error";
      quran.error = e instanceof Error ? e.message : String(e);
    }
  })();

  async function refreshBootMetadata(): Promise<void> {
    try {
      const [remoteManifest, remoteCatalogue] = await Promise.all([
        resolveManifest(),
        catalogueStore.ensure(),
      ]);
      if (!active) return;
      quran.source = remoteManifest.source;
      await quranWorker.provideCatalogue(remoteCatalogue);
    } catch {}
  }

  return () => {
    active = false;
    detachStatus();
    detachProgress();
    quranWorker.dispose();
  };
}
