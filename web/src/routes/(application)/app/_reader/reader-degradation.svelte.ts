import type { ReadTierStatus } from "$lib/quran/fetch";

/**
 * Tracks the "something failed to load" state for a SurahReader instance:
 * a specific page that failed to fetch, plus worker/API degradation flags
 * reported by the read-tier status callback. Grouped together because every
 * transition (page load success/failure, tier status, route change) touches
 * more than one of these fields at once.
 */
export class ReaderDegradationState {
  loadFailed = $state(false);
  failedPage = $state<number | null>(null);
  workerDegraded = $state(false);
  apiDegraded = $state(false);

  /** Clear everything, e.g. when the route (surah/page/source) changes. */
  reset(): void {
    this.workerDegraded = false;
    this.apiDegraded = false;
    this.loadFailed = false;
    this.failedPage = null;
  }

  applyTierStatus(status: ReadTierStatus): void {
    this.workerDegraded = !!status.workerFailure;
    this.apiDegraded = !!status.apiFailure;
  }

  markPageFailed(localPage: number): void {
    this.loadFailed = true;
    this.failedPage = localPage;
  }

  /** Clear failure only if it belongs to the page that just succeeded. */
  clearIfMatches(localPage: number): void {
    if (this.failedPage === localPage) {
      this.failedPage = null;
      this.loadFailed = false;
    }
  }
}
