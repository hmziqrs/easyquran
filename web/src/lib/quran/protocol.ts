/*
   protocol.ts — the quran.worker RPC message types (type-only).

   Shared by the worker (quran.worker.ts) and its client (worker-client.ts) so
   both sides agree on the wire shape. Imported as types only, so it pulls no
   runtime code (and no $env/SvelteKit code) into the worker bundle.
*/

import type { DownloadProgress } from "$lib/data/quran-types";
import type { ResolvedManifest } from "./manifest";
import type { SearchOpts } from "./search/types";

export type WorkerStatus = "init" | "downloading" | "ready" | "error";

export type WorkerRequest =
  | { id: number; type: "init"; manifest: ResolvedManifest }
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
