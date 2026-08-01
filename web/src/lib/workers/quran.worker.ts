/* Offline Quran engine: validated registered SQLite sources in sqlite-wasm.

   The Worker only adapts sqlite-wasm to QuranQueryRunner and owns artifact
   lifecycle. Query definitions, row decoding, source validation, canonical
   views, and search-corpus construction are shared with SSG/tooling. */

import init, { type Database, type Sqlite3Static } from "@sqlite.org/sqlite-wasm";
import { type ArtifactSpec, type QuranSourceId, type QuranSurahText } from "$lib/data/quran-types";
import type { QuranQueryRunner } from "$lib/quran/sql";
import { DEFAULT_QURAN_SOURCE_PLAN, plannedSourceIds } from "$lib/quran/source-plan";
import { createWasmQueryRunner } from "$lib/quran/wasm-query-runner";
import { ensureArtifact } from "./opfs-cache";
import type { ResolvedManifest } from "../quran/manifest";
import type { WorkerOutbound, WorkerRequest, WorkerStatus } from "../quran/protocol";
import {
  buildCanonicalSearchCorpus,
  searchCanonicalCorpus,
  type CanonicalSearchUnit,
} from "../quran/search/corpus";
import { SearchProvider, type SearchOpts, type SearchResponse } from "../quran/search/types";
import {
  loadQuranSource,
  readAllSourceRows,
  readSourceSurah,
  type LoadedQuranSource,
} from "../quran/view/source-runtime";
import { resolveSourceProfile } from "../quran/view/source-profiles";

interface WorkerCtx {
  postMessage(msg: WorkerOutbound): void;
  onmessage: ((event: MessageEvent<WorkerRequest>) => void) | null;
}
const ctx = self as unknown as WorkerCtx;

let sqlite3: Sqlite3Static | null = null;

interface WorkerSourceState {
  readonly bytes: Uint8Array;
  readonly source: LoadedQuranSource;
  readonly store: string;
  /** Present for roles that need repeated reads; other sources reopen lazily. */
  readonly runner: QuranQueryRunner | null;
}

const sources = new Map<QuranSourceId, WorkerSourceState>();
let corpus: CanonicalSearchUnit[] | null = null;
let ready = false;

function emit(message: WorkerOutbound): void {
  ctx.postMessage(message);
}

function status(value: WorkerStatus, detail?: string): void {
  emit({ type: "status", status: value, detail });
}

function progressEmitter(spec: ArtifactSpec): (loaded: number, total: number) => void {
  let lastPct = -1;
  return (loaded, total) => {
    const pct = total > 0 ? Math.floor((loaded / total) * 100) : 0;
    if (pct === lastPct) return;
    lastPct = pct;
    emit({ type: "progress", script: spec.id, loaded, total });
  };
}

function openReadOnly(bytes: Uint8Array): Database {
  const database = new sqlite3!.oo1.DB();
  const flags =
    sqlite3!.capi.SQLITE_DESERIALIZE_FREEONCLOSE | sqlite3!.capi.SQLITE_DESERIALIZE_READONLY;
  const pointer = sqlite3!.wasm.allocFromTypedArray(bytes);
  const result = sqlite3!.capi.sqlite3_deserialize(
    database,
    "main",
    pointer,
    bytes.byteLength,
    bytes.byteLength,
    flags,
  );
  if (result !== sqlite3!.capi.SQLITE_OK) {
    sqlite3!.wasm.dealloc(pointer);
    throw new Error(`sqlite3_deserialize failed: rc=${result}`);
  }
  return database;
}

async function initialize(manifest: ResolvedManifest): Promise<void> {
  status("init");
  sqlite3 = await init();

  const persistentSources = new Set([
    DEFAULT_QURAN_SOURCE_PLAN.reader,
    DEFAULT_QURAN_SOURCE_PLAN.search.display,
  ]);
  for (const sourceId of plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN)) {
    const spec = manifest.scripts.find((artifact) => artifact.id === sourceId);
    if (!spec) throw new Error(`manifest missing Quran source ${sourceId}`);
    const profile = resolveSourceProfile(spec.id, spec.sha256);
    status("downloading", sourceId);
    const artifact = await ensureArtifact(spec, manifest.contentVersion, progressEmitter(spec));
    const database = openReadOnly(artifact.bytes);
    const runner = createWasmQueryRunner(database);
    const source = loadQuranSource(runner, profile);
    const persistent = persistentSources.has(sourceId);
    sources.set(sourceId, {
      bytes: artifact.bytes,
      source,
      store: artifact.store,
      runner: persistent ? runner : null,
    });
    if (!persistent) database.close();
  }

  ready = true;
  const loaded = [...sources].map(([id, state]) => `${id}: ${state.store}`).join(", ");
  console.info(
    `[quran] offline engine ready (${loaded}); ` +
      `surah 1 has ${readSurah(1).verses.length} verses`,
  );
  status("ready");
}

function readSurah(num: number): QuranSurahText {
  const state = sourceState(DEFAULT_QURAN_SOURCE_PLAN.reader);
  if (!state.runner) throw new Error("reader source is not open");
  return {
    sourceId: state.source.profile.sourceId,
    script: state.source.profile.script,
    verses: readSourceSurah(state.runner, state.source, num),
    normalization: state.source.view.normalization(num),
  };
}

function sourceState(sourceId: QuranSourceId): WorkerSourceState {
  const state = sources.get(sourceId);
  if (!state) throw new Error(`Quran source ${sourceId} is not loaded`);
  return state;
}

function readAllRows(state: WorkerSourceState) {
  if (state.runner) return readAllSourceRows(state.runner, state.source);
  const database = openReadOnly(state.bytes);
  try {
    return readAllSourceRows(createWasmQueryRunner(database), state.source);
  } finally {
    database.close();
  }
}

function ensureSearchCorpus(): void {
  if (corpus) return;
  const match = sourceState(DEFAULT_QURAN_SOURCE_PLAN.search.match);
  const display = sourceState(DEFAULT_QURAN_SOURCE_PLAN.search.display);
  corpus = buildCanonicalSearchCorpus({
    matchRows: readAllRows(match),
    displayRows: readAllRows(display),
    matchView: match.source.view,
    displayView: display.source.view,
  });
}

function search(query: string, opts: SearchOpts = {}): SearchResponse {
  ensureSearchCorpus();
  return {
    query,
    ...searchCanonicalCorpus(corpus!, query, opts),
    source: SearchProvider.Worker,
  };
}

ctx.onmessage = async (event: MessageEvent<WorkerRequest>): Promise<void> => {
  const message = event.data;
  const id = message.id;
  const type = (message as { type: string }).type;
  try {
    if (message.type === "init") {
      await initialize(message.manifest);
      emit({ id, ok: true, result: null });
      emit({ type: "ready" });
      return;
    }
    if (!ready) {
      emit({ id, ok: false, error: "engine not ready" });
      return;
    }
    if (message.type === "readSurah") {
      emit({ id, ok: true, result: readSurah(message.num) });
    } else if (message.type === "search") {
      emit({ id, ok: true, result: search(message.query, message.opts) });
    } else if (message.type === "ping") {
      emit({ id, ok: true, result: "pong" });
    } else {
      emit({ id, ok: false, error: `unknown request ${type}` });
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    if (type === "init") {
      status("error", errorMessage);
      emit({ type: "fatal", error: errorMessage });
    }
    emit({ id, ok: false, error: errorMessage });
  }
};
