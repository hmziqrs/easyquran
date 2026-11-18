import { offline } from "#lib/offline/offline-store.svelte.js";
import { quran } from "#lib/stores/quran.svelte.js";

/**
 * Which download run the progress popup was hidden for. Shared by the popup and
 * the /downloads screen. Kept in memory only: hiding lasts for the current run,
 * and a new run (the next Quran or pack download, or a reload) shows it again.
 * Hiding never stops a download.
 */
class DownloadDismissal {
  #quranRun = $state<number | null>(null);
  #packRun = $state<number | null>(null);

  get quranHidden(): boolean {
    return this.#quranRun === quran.downloadRun;
  }

  get packHidden(): boolean {
    return this.#packRun === offline.run;
  }

  hide(): void {
    this.#quranRun = quran.downloadRun;
    this.#packRun = offline.run;
  }

  show(): void {
    this.#quranRun = null;
    this.#packRun = null;
  }
}

export const downloadDismissal = new DownloadDismissal();
