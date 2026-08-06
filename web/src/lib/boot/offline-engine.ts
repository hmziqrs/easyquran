import { bootOfflineEngine } from "$lib/quran/offline";
import { quranWorker } from "$lib/quran/worker-client";
import { catalogueStore } from "$lib/quran/catalogue-store.svelte";

let started = false;
let teardown: (() => void) | null = null;

export function startOfflineEngine(): () => void {
  if (started) return () => {};
  started = true;
  teardown = bootOfflineEngine();
  void catalogueStore.ensure()
    .then((entries) => {
      void quranWorker.provideCatalogue(entries);
    })
    .catch(() => {});
  return () => {
    teardown?.();
    teardown = null;
    started = false;
  };
}
