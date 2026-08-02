import type { CanonicalQuranCoordinates, DownloadProgress } from "$lib/data/quran-types";
import type { ResolvedManifest } from "./manifest";
import type { SearchOpts } from "./search/types";

export type WorkerStatus = "init" | "downloading" | "ready" | "error";

export type WorkerRequest =
  | {
      id: number;
      type: "init";
      manifest: ResolvedManifest;
      coordinates: CanonicalQuranCoordinates;
    }
  | { id: number; type: "readSurah"; num: number }
  | { id: number; type: "search"; query: string; opts?: SearchOpts }
  | { id: number; type: "ping" };

export type WorkerResponse =
  | { id: number; ok: true; result: unknown }
  | { id: number; ok: false; error: string };

export type WorkerEvent =
  | { type: "status"; status: WorkerStatus; detail?: string }
  | { type: "ready" }
  | { type: "fatal"; error: string }
  | ({ type: "progress" } & DownloadProgress);

export type WorkerOutbound = WorkerResponse | WorkerEvent;
