import { browser } from "$app/env";

import type { DownloadProgress } from "#lib/data/quran-types.js";
import type { WorkerStatus } from "#lib/quran/protocol.js";

export type QuranStatus = "idle" | "resolving" | "init" | "downloading" | "ready" | "error";

class QuranStore {
  status = $state<QuranStatus>("idle");
  detail = $state<string>("");
  error = $state<string | null>(null);
  download = $state<DownloadProgress | null>(null);
  /** Bumps when a download starts after none was in flight: one id per download run (progress-popup dismissal keys on it). */
  downloadRun = $state(0);

  setWorkerStatus(s: WorkerStatus, detail?: string): void {
    if (s === "ready") {
      this.status = "ready";
      this.detail = "";
      this.error = null;
      this.download = null;
    } else if (s === "error") {
      this.status = "error";
      this.error = detail ?? "offline data error";
      this.download = null;
    } else if (s === "translation-fetch-failed") {
      return;
    } else {
      this.status = s;
      if (detail) this.detail = detail;
    }
  }

  setDownload(p: DownloadProgress): void {
    if (this.download === null) this.downloadRun += 1;
    this.download = p;
  }

  get downloadPct(): number | null {
    const d = this.download;
    return d && d.total > 0 ? Math.min(1, d.loaded / d.total) : null;
  }

  get offlineReady(): boolean {
    return this.status === "ready";
  }
  get online(): boolean {
    return browser ? navigator.onLine : true;
  }
}

export const quran = new QuranStore();
