import { isArabicSourceId } from "$lib/data/quran-types";
import type {
  CanonicalQuranCoordinates,
  DownloadProgress,
  QuranRangeText,
  QuranReaderSource,
  QuranSurahText,
  SourceCatalogueEntry,
} from "$lib/data/quran-types";
import { QURAN } from "$lib/config/site";
import { quranApi } from "./api-client";
import { DEFAULT_QURAN_SOURCE_PLAN } from "./source-plan";
import type { ResolvedManifest } from "./manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "./protocol";
import { DEFAULT_LIMIT, DEFAULT_OFFSET } from "./search/normalize";
import { SearchProvider, type SearchOpts, type SearchResponse } from "./search/types";
import {
  decodeQuranRangeText,
  decodeQuranSurahText,
  decodeSearchResponse,
  decodeTranslationRangeText,
  decodeTranslationSurahText,
  type AyahCoordinateValidator,
} from "./wire";

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
let stashedManifest: ResolvedManifest | null = null;
let stashedCoordinates: CanonicalQuranCoordinates | null = null;

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

function resetWorker(error?: Error): void {
  if (error) failAll(error);
  worker?.terminate();
  worker = null;
  isReady = false;
  startPromise = null;
  stashedManifest = null;
  stashedCoordinates = null;
}

function reportWorkerFailure(error: Error): void {
  resetWorker(error);
  for (const listener of statusListeners) listener("error", error.message);
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
    if (msg.status === "ready") isReady = true;
    for (const cb of statusListeners) cb(msg.status, msg.detail);
  } else if (msg.type === "progress") {
    const p: DownloadProgress = { script: msg.script, loaded: msg.loaded, total: msg.total };
    for (const cb of progressListeners) cb(p);
  } else if (msg.type === "fatal") {
    reportWorkerFailure(new Error(msg.error));
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

  whenReady(): Promise<void> {
    if (isReady) return Promise.resolve();
    if (startPromise) return startPromise;
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        statusListeners.delete(listener);
        reject(new Error("quran worker did not become ready"));
      }, DEFAULT_TIMEOUT_MS);
      const listener = (status: WorkerStatus, detail?: string) => {
        if (status !== "ready" && status !== "error") return;
        clearTimeout(timer);
        statusListeners.delete(listener);
        if (status === "ready") resolve();
        else reject(new Error(detail ?? "quran worker failed to start"));
      };
      statusListeners.add(listener);
    });
  },

  start(
    manifest: ResolvedManifest,
    coordinates: CanonicalQuranCoordinates,
    catalogue?: readonly SourceCatalogueEntry[],
  ): Promise<void> {
    if (startPromise) return startPromise;
    stashedManifest = manifest;
    stashedCoordinates = coordinates;
    const attempt = (async () => {
      worker = new Worker(new URL("../workers/quran.worker.ts", import.meta.url), {
        type: "module",
        name: "quran-db",
      });
      worker.addEventListener("message", (e: MessageEvent<WorkerOutbound>) => handle(e.data));
      worker.addEventListener("error", (e: ErrorEvent) => {
        reportWorkerFailure(
          e.error instanceof Error
            ? e.error
            : new Error(`quran worker failed to load: ${e.message}`),
        );
      });
      worker.addEventListener("messageerror", () => {
        reportWorkerFailure(new Error("quran worker message could not be deserialized"));
      });
      await request<null>((id) => ({
        id,
        type: "init",
        manifest,
        coordinates,
        catalogue: catalogue ? [...catalogue] : undefined,
      }));
      isReady = true;
    })();
    startPromise = attempt;
    void attempt.catch((error: unknown) => {
      if (startPromise === attempt) {
        resetWorker(error instanceof Error ? error : new Error(String(error)));
      }
    });
    return attempt;
  },

  provideCatalogue(catalogue: readonly SourceCatalogueEntry[]): Promise<void> {
    return quranWorker
      .whenReady()
      .then(() => {
        if (!stashedManifest || !stashedCoordinates) return null;
        const manifest = stashedManifest;
        const coordinates = stashedCoordinates;
        return request<null>((id) => ({
          id,
          type: "init",
          manifest,
          coordinates,
          catalogue: [...catalogue],
        }));
      })
      .then(() => undefined)
      .catch(() => undefined);
  },

  dispose(): void {
    resetWorker(new Error("quran worker disposed"));
  },

  readSurah(num: number, source?: QuranReaderSource): Promise<QuranSurahText> {
    const reader = source ?? DEFAULT_QURAN_SOURCE_PLAN.reader;
    return request<QuranSurahText>((id) => ({ id, type: "readSurah", num, source }))
      .then((raw: unknown) => {
        const decoded = isArabicSourceId(reader)
          ? decodeQuranSurahText(raw)
          : decodeTranslationSurahText(raw);
        if (!decoded) throw new Error("quran worker returned a malformed surah");
        return decoded;
      })
      .catch((err) => {
        if (QURAN.apiBase) return quranApi.readSurah(reader, num);
        throw err;
      });
  },

  readRange(
    from: number,
    to: number,
    validateCoordinate?: AyahCoordinateValidator,
    source?: QuranReaderSource,
  ): Promise<QuranRangeText> {
    const reader = source ?? DEFAULT_QURAN_SOURCE_PLAN.reader;
    return request<QuranRangeText>((id) => ({ id, type: "readRange", from, to, source })).then(
      (raw: unknown) => {
        const decoded = isArabicSourceId(reader)
          ? decodeQuranRangeText(raw, validateCoordinate)
          : decodeTranslationRangeText(raw, validateCoordinate);
        if (!decoded) throw new Error("quran worker returned a malformed range");
        return decoded;
      },
    )
      .catch((err) => {
        if (QURAN.apiBase) return quranApi.readRange(reader, from, to);
        throw err;
      });
  },

  search(
    query: string,
    opts?: SearchOpts,
    validateCoordinate?: AyahCoordinateValidator,
  ): Promise<SearchResponse> {
    return request<SearchResponse>((id) => ({ id, type: "search", query, opts })).then(
      (r: unknown) => {
        const limit = opts?.limit ?? DEFAULT_LIMIT;
        const offset = opts?.offset ?? DEFAULT_OFFSET;
        const payload = decodeSearchResponse(r, validateCoordinate);
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
