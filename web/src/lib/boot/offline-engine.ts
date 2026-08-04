import { bootOfflineEngine } from "$lib/quran/offline";
import { quranWorker } from "$lib/quran/worker-client";
import { resolveSourceCatalogue } from "$lib/quran/catalogue";

let started = false;
let teardown: (() => void) | null = null;

export function startOfflineEngine(): () => void {
  if (started) return () => {};
  started = true;
  teardown = bootOfflineEngine();
  void resolveSourceCatalogue()
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
