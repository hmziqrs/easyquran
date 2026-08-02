import type { DownloadProgress, QuranSurahText } from "$lib/data/quran-types";
import { QURAN } from "$lib/config/site";
import { quranApi } from "./api-client";
import { DEFAULT_QURAN_SOURCE_PLAN } from "./source-plan";
import type { ResolvedManifest } from "./manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "./protocol";
import { DEFAULT_LIMIT, DEFAULT_OFFSET } from "./search/normalize";
import { SearchProvider, type SearchOpts, type SearchResponse } from "./search/types";
import { decodeQuranSurahText, decodeSearchResponse } from "./wire";

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

const DEFAULT_TIMEOUT_MS = 30_000;

let worker: Worker | null = null;
let seq = 0;
let isReady = false;
let startPromise: Promise<void> | null = null;

const pending = new Map<number, Pending>();
const statusListeners = new Set<(s: WorkerStatus, detail?: string) => void>();
const progressListeners = new Set<(p: DownloadProgress) => void>();

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
    if (!p) return;
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
  onStatus(cb: (s: WorkerStatus, detail?: string) => void): () => void {
    statusListeners.add(cb);
    return () => statusListeners.delete(cb);
  },
  onProgress(cb: (p: DownloadProgress) => void): () => void {
    progressListeners.add(cb);
    return () => progressListeners.delete(cb);
  },

  start(manifest: ResolvedManifest): Promise<void> {
    if (startPromise) return startPromise;
    startPromise = (async () => {
      worker = new Worker(new URL("../workers/quran.worker.ts", import.meta.url), {
        type: "module",
        name: "quran-db",
      });
      worker.addEventListener("message", (e: MessageEvent<WorkerOutbound>) => handle(e.data));
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

  dispose(): void {
    if (!worker) return;
    failAll(new Error("quran worker disposed"));
    worker.terminate();
    worker = null;
    isReady = false;
    startPromise = null;
  },

  readSurah(num: number): Promise<QuranSurahText> {
    return request<QuranSurahText>((id) => ({ id, type: "readSurah", num }))
      .then((raw: unknown) => {
        const decoded = decodeQuranSurahText(raw);
        if (!decoded) throw new Error("quran worker returned a malformed surah");
        return decoded;
      })
      .catch((err) => {
        if (QURAN.apiBase) return quranApi.readSurah(DEFAULT_QURAN_SOURCE_PLAN.reader, num);
        throw err;
      });
  },

  search(query: string, opts?: SearchOpts): Promise<SearchResponse> {
    return request<SearchResponse>((id) => ({ id, type: "search", query, opts })).then(
      (r: unknown) => {
        const limit = opts?.limit ?? DEFAULT_LIMIT;
        const offset = opts?.offset ?? DEFAULT_OFFSET;
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
