import { isArabicSourceId } from "$lib/data/quran-types";
import type {
  Ayah,
  CanonicalQuranCoordinates,
  DownloadProgress,
  QuranRangeText,
  QuranReaderSource,
  QuranSurahText,
  SourceCatalogueEntry,
  SurahLink,
  SurahNormalization,
} from "$lib/data/quran-types";
import { QURAN } from "$lib/config/site";
import { quranApi } from "./api-client";
import { DEFAULT_QURAN_SOURCE_PLAN } from "./source-plan";
import {
  classifyApiFailure,
  classifyWorkerFailure,
  LOCAL_BOOT_BUDGET_MS,
  ReadChainError,
  type ReadFailure,
  type ReadTierStatus,
} from "./fetch";
import type { ResolvedManifest } from "./manifest";
import type { WorkerEvent, WorkerOutbound, WorkerRequest, WorkerStatus } from "./protocol";
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
}

function reportWorkerFailure(error: Error): void {
  resetWorker(error);
  for (const listener of statusListeners) listener("error", error.message);
}

const eventHandlers: {
  [K in WorkerEvent["type"]]: (msg: Extract<WorkerEvent, { type: K }>) => void;
} = {
  status: (m) => {
    if (m.status === "ready") isReady = true;
    for (const cb of statusListeners) cb(m.status, m.detail);
  },
  progress: (m) => {
    const p: DownloadProgress = { script: m.script, loaded: m.loaded, total: m.total };
    for (const cb of progressListeners) cb(p);
  },
  fatal: (m) => {
    reportWorkerFailure(new Error(m.error));
  },
};

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
  (eventHandlers[msg.type] as (m: WorkerEvent) => void)(msg);
}

function request<T>(
  build: (id: number) => WorkerRequest,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  const activeWorker = worker;
  if (!activeWorker) return Promise.reject(new Error("quran worker not started"));
  const id = ++seq;
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (pending.delete(id)) reject(new Error("quran worker request timed out"));
    }, timeoutMs);
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
    activeWorker.postMessage(build(id));
  });
}

function waitForBootBudget(): Promise<void> {
  if (isReady || !startPromise) return Promise.resolve();
  return new Promise<void>((resolve) => {
    let settled = false;
    const finish = (): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      statusListeners.delete(listener);
      resolve();
    };
    const timer = setTimeout(finish, LOCAL_BOOT_BUDGET_MS);
    const listener = (status: WorkerStatus): void => {
      if (status === "ready" || status === "error") finish();
    };
    statusListeners.add(listener);
    void startPromise?.then(finish, finish);
  });
}

interface SourceFallbackArgs<T> {
  hasLocal: () => boolean | Promise<boolean>;
  onMiss?: () => void;
  workerReq: (id: number) => WorkerRequest;
  decode: (raw: unknown) => T | null;
  apiFetch: () => Promise<T>;
  errorMessage: string;
  onStatus?: (status: ReadTierStatus) => void;
}

async function withSourceFallback<T>(args: SourceFallbackArgs<T>): Promise<T> {
  let workerFailure: ReadFailure | undefined;
  let apiFailure: ReadFailure | undefined;
  let workerError: unknown;

  if (!isReady && startPromise) await waitForBootBudget();

  const tryLocal = async (): Promise<{ value: T } | null> => {
    let local: boolean;
    const probe = args.hasLocal();
    if (typeof probe === "boolean") {
      local = probe;
    } else {
      try {
        local = await probe;
      } catch (e) {
        if (!workerFailure) {
          workerFailure = classifyWorkerFailure(e);
          workerError = e;
        }
        return null;
      }
    }
    if (!local) return null;
    try {
      const decoded = args.decode(await request<unknown>(args.workerReq));
      if (decoded) return { value: decoded };
      if (!workerFailure) workerFailure = { kind: "malformed" };
    } catch (e) {
      if (!workerFailure) {
        workerFailure = classifyWorkerFailure(e);
        workerError = e;
      }
    }
    return null;
  };

  const fail = (): never => {
    const detail = workerError instanceof Error ? `: ${workerError.message}` : "";
    args.onStatus?.({ servedBy: apiFailure ? "api" : "local", workerFailure, apiFailure });
    throw new ReadChainError(`${args.errorMessage}${detail}`, workerFailure, apiFailure);
  };

  const first = await tryLocal();
  if (first) {
    args.onStatus?.({ servedBy: "local" });
    return first.value;
  }

  args.onMiss?.();

  if (QURAN.apiBase) {
    try {
      const result = await args.apiFetch();
      args.onStatus?.({ servedBy: "api", workerFailure });
      return result;
    } catch (e) {
      apiFailure = classifyApiFailure(e);
    }
    const recheck = await tryLocal();
    if (recheck) {
      args.onStatus?.({ servedBy: "local", apiFailure });
      return recheck.value;
    }
  }

  return fail();
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

  hasTranslation(source: QuranReaderSource): Promise<boolean> {
    return request<boolean>((id) => ({ id, type: "hasTranslation", source }));
  },
  ensureTranslation(source: QuranReaderSource): Promise<void> {
    return request<null>((id) => ({ id, type: "ensureTranslation", source })).then(() => undefined);
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
      .then(() =>
        request<null>((id) => ({
          id,
          type: "refreshCatalogue",
          catalogue: [...catalogue],
        })),
      )
      .then(() => undefined)
      .catch((err: unknown) =>
        reportWorkerFailure(err instanceof Error ? err : new Error(String(err))),
      );
  },

  dispose(): void {
    resetWorker(new Error("quran worker disposed"));
  },

  async readSurah(
    num: number,
    source?: QuranReaderSource,
    onStatus?: (status: ReadTierStatus) => void,
  ): Promise<QuranSurahText> {
    const reader = source ?? DEFAULT_QURAN_SOURCE_PLAN.reader;
    if (isArabicSourceId(reader)) {
      return withSourceFallback({
        hasLocal: () => quranWorker.ready,
        workerReq: (id) => ({ id, type: "readSurah", num, source: reader }),
        decode: decodeQuranSurahText,
        apiFetch: () => quranApi.readSurah(reader, num),
        errorMessage: `arabic surah unavailable: ${reader}/${num}`,
        onStatus,
      });
    }
    return withSourceFallback({
      hasLocal: () => (quranWorker.ready ? quranWorker.hasTranslation(reader) : false),
      onMiss: () => {
        void quranWorker.ensureTranslation(reader).catch(() => {});
      },
      workerReq: (id) => ({ id, type: "readSurah", num, source: reader }),
      decode: decodeTranslationSurahText,
      apiFetch: () => quranApi.readSurah(reader, num),
      errorMessage: `translation surah unavailable: ${reader}/${num}`,
      onStatus,
    });
  },

  async readRange(
    from: number,
    to: number,
    validateCoordinate?: AyahCoordinateValidator,
    source?: QuranReaderSource,
    onStatus?: (status: ReadTierStatus) => void,
  ): Promise<QuranRangeText> {
    const reader = source ?? DEFAULT_QURAN_SOURCE_PLAN.reader;
    if (isArabicSourceId(reader)) {
      return withSourceFallback({
        hasLocal: () => quranWorker.ready,
        workerReq: (id) => ({ id, type: "readRange", from, to, source: reader }),
        decode: (raw) => decodeQuranRangeText(raw, validateCoordinate),
        apiFetch: () => quranApi.readRange(reader, from, to, undefined, validateCoordinate),
        errorMessage: `arabic range unavailable: ${reader} ${from}-${to}`,
        onStatus,
      });
    }
    return withSourceFallback({
      hasLocal: () => (quranWorker.ready ? quranWorker.hasTranslation(reader) : false),
      onMiss: () => {
        void quranWorker.ensureTranslation(reader).catch(() => {});
      },
      workerReq: (id) => ({ id, type: "readRange", from, to, source: reader }),
      decode: (raw) => decodeTranslationRangeText(raw, validateCoordinate),
      apiFetch: () => quranApi.readRange(reader, from, to, undefined, validateCoordinate),
      errorMessage: `translation range unavailable: ${reader} ${from}-${to}`,
      onStatus,
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

export interface RangeRouteKey {
  readonly sourceId: string | null;
  readonly kind: "juz" | "page";
  readonly index: number;
}

export interface RangeDisplaySnapshot {
  readonly ayahs: readonly Ayah[];
  readonly normalizations: readonly SurahNormalization[];
  readonly surahs: readonly SurahLink[];
}

export interface RangeServerInstall {
  readonly read: boolean;
}

export interface RangeClientApply {
  readonly applied: boolean;
}

export interface RangeReaderCoordinator {
  installServer(key: RangeRouteKey, snapshot: RangeDisplaySnapshot): RangeServerInstall;
  applyClientResult(key: RangeRouteKey, snapshot: RangeDisplaySnapshot): RangeClientApply;
  markFailed(key: RangeRouteKey, failure: ReadFailure | undefined): void;
  canRetry(): RangeRouteKey | null;
  currentSnapshot(): RangeDisplaySnapshot;
  currentKey(): RangeRouteKey | null;
  isDegraded(): boolean;
  lastFailure(): ReadFailure | undefined;
}

export function rangeRouteKey(
  sourceId: string | null,
  kind: "juz" | "page",
  index: number,
): RangeRouteKey {
  return { sourceId, kind, index };
}

export function equalRangeKey(a: RangeRouteKey, b: RangeRouteKey): boolean {
  return a.sourceId === b.sourceId && a.kind === b.kind && a.index === b.index;
}

const EMPTY_RANGE_SNAPSHOT: RangeDisplaySnapshot = {
  ayahs: [],
  normalizations: [],
  surahs: [],
};

export function createRangeReaderCoordinator(): RangeReaderCoordinator {
  let currentKey: RangeRouteKey | null = null;
  let displayed: RangeDisplaySnapshot | null = null;
  let degraded = false;
  let failure: ReadFailure | undefined;

  return {
    installServer(key, snapshot) {
      const prevKey = currentKey;
      currentKey = key;
      displayed = snapshot;
      degraded = snapshot.ayahs.length === 0;
      failure = undefined;
      let read: boolean;
      if (prevKey === null) {
        read = degraded;
      } else if (equalRangeKey(prevKey, key)) {
        read = degraded;
      } else {
        read = true;
      }
      return { read };
    },
    applyClientResult(key, snapshot) {
      if (currentKey === null || !equalRangeKey(key, currentKey)) {
        return { applied: false };
      }
      displayed = snapshot;
      degraded = false;
      failure = undefined;
      return { applied: true };
    },
    markFailed(key, f) {
      if (currentKey === null || !equalRangeKey(key, currentKey)) return;
      degraded = displayed ? displayed.ayahs.length === 0 : true;
      failure = f;
    },
    canRetry() {
      if (currentKey !== null && degraded) return currentKey;
      return null;
    },
    currentSnapshot() {
      return displayed ?? EMPTY_RANGE_SNAPSHOT;
    },
    currentKey() {
      return currentKey;
    },
    isDegraded() {
      return degraded;
    },
    lastFailure() {
      return failure;
    },
  };
}
