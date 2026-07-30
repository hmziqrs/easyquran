/* ════════════════════════════════════════════════════════════════════════
   quran.svelte.ts — offline-data status for the Quran reader.

   A small runes store that mirrors the worker lifecycle so the UI can show a
   status pill (downloading / offline-ready / offline-Arabic-only). Verse
   RENDERING never reads this — the reader always paints instantly from the
   prerendered page.data / sync cache; this store only describes the offline
   engine's background readiness.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { WorkerStatus } from "$lib/quran/protocol";

export type QuranStatus =
  | "idle" // not yet started
  | "resolving" // resolving the manifest
  | "init" // worker loading wasm
  | "downloading" // fetching/caching the DBs
  | "ready" // engine online; full offline + local reads
  | "error";

class QuranStore {
  status = $state<QuranStatus>("idle");
  /** what the worker is currently working on (e.g. "uthmani"). */
  detail = $state<string>("");
  source = $state<"unknown" | "baked" | "api">("unknown");
  error = $state<string | null>(null);

  /** Map worker lifecycle events onto this store. */
  setWorkerStatus(s: WorkerStatus, detail?: string): void {
    if (s === "ready") {
      this.status = "ready";
      this.detail = "";
      this.error = null;
    } else if (s === "error") {
      this.status = "error";
      this.error = detail ?? "offline data error";
    } else {
      this.status = s; // init | downloading
      if (detail) this.detail = detail;
    }
  }

  get offlineReady(): boolean {
    return this.status === "ready";
  }
  get online(): boolean {
    return browser ? navigator.onLine : true;
  }
}

export const quran = new QuranStore();
