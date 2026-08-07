import { browser } from "$app/environment";
import type { DownloadProgress } from "$lib/data/quran-types";
import type { WorkerStatus } from "$lib/quran/protocol";

export type QuranStatus = "idle" | "resolving" | "init" | "downloading" | "ready" | "error";

class QuranStore {
  status = $state<QuranStatus>("idle");
  detail = $state<string>("");
  source = $state<"unknown" | "baked" | "api">("unknown");
  error = $state<string | null>(null);
  download = $state<DownloadProgress | null>(null);

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
