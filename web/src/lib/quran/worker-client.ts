/* ════════════════════════════════════════════════════════════════════════
   worker-client.ts — main-thread proxy for the quran.worker.

   Owns the Worker lifecycle, request/response correlation, and lifecycle-event
   forwarding. The only place `new Worker(...)` is constructed, so the worker
   bundle stays out of the server/prerender graph. `start()` is called from a
   browser-only onMount with a resolved manifest (manifest.ts runs here, on the
   main thread — never inside the worker).

   Every request settles deterministically: on a response, on a per-request
   timeout, on a worker load/eval/runtime fatal, or on disposal. No in-flight
   request can hang forever.
   ════════════════════════════════════════════════════════════════════════ */

import type { DownloadProgress, QuranSurahText } from "$lib/data/quran-types";
import type { ResolvedManifest } from "./manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "./protocol";
import { DEFAULT_LIMIT, DEFAULT_OFFSET } from "./search/normalize";
import { SearchProvider, type SearchOpts, type SearchResponse } from "./search/types";
import { decodeQuranSurahText, decodeSearchResponse } from "./wire";

/** Per-request settlement handle. The timer is cleared on every settle path
 *  (response, timeout, fatal, disposal) so no dangling rejection fires later. */
interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

/** Requests that never get a worker response must still settle. Generous: the
 *  corpus is local (OPFS), so this only trips on a genuinely stuck worker. */
const DEFAULT_TIMEOUT_MS = 30_000;

let worker: Worker | null = null;
let seq = 0;
let isReady = false;
let startPromise: Promise<void> | null = null;

const pending = new Map<number, Pending>();
const statusListeners = new Set<(s: WorkerStatus, detail?: string) => void>();
const progressListeners = new Set<(p: DownloadProgress) => void>();

/** Reject every in-flight request (worker load/eval failure, a post-init fatal
 *  message, or disposal). Module-scoped so disposal and the fatal handler share
 *  exactly one cleanup path. */
function failAll(err: Error): void {
  for (const p of pending.values()) {
    clearTimeout(p.timer);
    p.reject(err);
  }
  pending.clear();
}

function handle(msg: WorkerOutbound): void {
  if ("id" in msg) {
    const p = pending.get(msg.id);
    if (!p) return; // already settled by timeout/disposal
    clearTimeout(p.timer);
    pending.delete(msg.id);
    if (msg.ok) p.resolve(msg.result);
    else p.reject(new Error(msg.error));
    return;
  }
  if (msg.type === "status") {
    for (const cb of statusListeners) cb(msg.status, msg.detail);
  } else if (msg.type === "progress") {
    const p: DownloadProgress = { script: msg.script, loaded: msg.loaded, total: msg.total };
    for (const cb of progressListeners) cb(p);
  } else if (msg.type === "fatal") {
    // A runtime fatal (e.g. a sqlite-wasm crash) can arrive AFTER init has
    // succeeded; reject every in-flight request too, not just readiness —
    // otherwise readSurah/search callers hang forever.
    failAll(new Error(msg.error));
  }
}

function request<T>(
  build: (id: number) => WorkerRequest,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  if (!worker) return Promise.reject(new Error("quran worker not started"));
  const id = ++seq;
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pending.delete(id)) reject(new Error("quran worker request timed out"));
    }, timeoutMs);
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
    worker!.postMessage(build(id));
  });
}

export const quranWorker = {
  get ready(): boolean {
    return isReady;
  },
  /** Subscribe to lifecycle status (init/downloading/ready/error). Returns an unsub. */
  onStatus(cb: (s: WorkerStatus, detail?: string) => void): () => void {
    statusListeners.add(cb);
    return () => statusListeners.delete(cb);
  },
  /** Subscribe to live download progress (per artifact). Returns an unsub. */
  onProgress(cb: (p: DownloadProgress) => void): () => void {
    progressListeners.add(cb);
    return () => progressListeners.delete(cb);
  },

  /** Start the worker with a resolved manifest. Idempotent; safe to call once. */
  start(manifest: ResolvedManifest): Promise<void> {
    if (startPromise) return startPromise;
    startPromise = (async () => {
      worker = new Worker(new URL("../workers/quran.worker.ts", import.meta.url), {
        type: "module",
        name: "quran-db",
      });
      worker.addEventListener("message", (e: MessageEvent<WorkerOutbound>) => handle(e.data));
      // A worker-bundle load/eval failure or an undeserializable message must
      // settle every pending request (including the init await below) — start()
      // then rejects via that init request instead of hanging forever.
      worker.addEventListener("error", (e: ErrorEvent) => {
        failAll(
          e.error instanceof Error
            ? e.error
            : new Error(`quran worker failed to load: ${e.message}`),
        );
      });
      worker.addEventListener("messageerror", () => {
        failAll(new Error("quran worker message could not be deserialized"));
      });
      try {
        await request<null>((id) => ({ id, type: "init", manifest }));
        isReady = true;
      } catch (err) {
        throw err instanceof Error ? err : new Error(String(err));
      }
    })();
    return startPromise;
  },

  /** Tear the worker down and reject every in-flight request with a disposal
   *  error. Resets module state so a later start() can spin up a fresh worker.
   *  No-op when not started. Has no production caller (the worker is page
   *  lifetime) but is required for test isolation and any future reader unmount. */
  dispose(): void {
    if (!worker) return;
    failAll(new Error("quran worker disposed"));
    worker.terminate();
    worker = null;
    isReady = false;
    startPromise = null;
  },

  /** Read raw Uthmani verses and their canonical view descriptor. */
  readSurah(num: number): Promise<QuranSurahText> {
    return request<QuranSurahText>((id) => ({ id, type: "readSurah", num })).then(
      (raw: unknown) => {
        const decoded = decodeQuranSurahText(raw);
        if (!decoded) throw new Error("quran worker returned a malformed surah");
        return decoded;
      },
    );
  },

  /** Substring search over the local normalized corpus. */
  search(query: string, opts?: SearchOpts): Promise<SearchResponse> {
    return request<SearchResponse>((id) => ({ id, type: "search", query, opts })).then(
      (r: unknown) => {
        const limit = opts?.limit ?? DEFAULT_LIMIT;
        const offset = opts?.offset ?? DEFAULT_OFFSET;
        // The worker boundary is structuredClone'd `unknown`, not a typed RPC:
        // rebuild via the shared wire decoder and fail closed on contract drift.
        const payload = decodeSearchResponse(r);
        if (!payload) throw new Error("quran worker returned a malformed search response");
        return {
          query,
          total: payload.total ?? payload.results.length,
          limit: payload.limit ?? limit,
          offset: payload.offset ?? offset,
          results: payload.results,
          source: SearchProvider.Worker,
        };
      },
    );
  },
};
