import init, { type Database, type Sqlite3Static } from "@sqlite.org/sqlite-wasm";
import {
  isArabicSourceId,
  OpenerKind,
  OpenerPackaging,
  QuranScript,
  type CanonicalQuranCoordinates,
  type DownloadableSpec,
  type QuranRangeText,
  type QuranReaderSource,
  type QuranSourceId,
  type QuranSurahText,
  type SourceCatalogueEntry,
  type SurahNormalization,
} from "$lib/data/quran-types";
import {
  runOne,
  runQuery,
  TANZIL_QURAN_DATABASE,
  type CanonicalQuranRow,
  type QuranCoordinateRow,
  type QuranQueryRunner,
} from "$lib/quran/sql";
import { DEFAULT_QURAN_SOURCE_PLAN, plannedSourceIds } from "$lib/quran/source-plan";
import { createWasmQueryRunner } from "$lib/quran/wasm-query-runner";
import {
  ensureArtifact,
  listCachedArtifacts,
  QURAN_ROW_COUNT,
  type StagedValidator,
} from "./opfs-cache";
import { pruneTranslations } from "./opfs-retention";
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
  readSourceRange,
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
  readonly runner: QuranQueryRunner | null;
}

const sources = new Map<QuranSourceId, WorkerSourceState>();
let corpus: CanonicalSearchUnit[] | null = null;
let ready = false;
let storedCatalogue: SourceCatalogueEntry[] = [];
let bootPromise: Promise<void> | null = null;
const TRANSLATION_DB_CAP = 4;
const translationDbs = new Map<string, Database>();
const pendingTranslationRunners = new Map<string, Promise<QuranQueryRunner>>();
let activeTranslationFetches = 0;
const cachedTranslationIds = new Set<string>();
const PINNED_ARABIC: readonly string[] = Object.freeze(plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN));

function emit(message: WorkerOutbound): void {
  ctx.postMessage(message);
}

function status(value: WorkerStatus, detail?: string): void {
  emit({ type: "status", status: value, detail });
}

function progressEmitter(spec: DownloadableSpec): (loaded: number, total: number) => void {
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

export function assertStagedQuranContent(
  count: number,
  rows: readonly QuranCoordinateRow[],
  sourceId: string,
): void {
  if (count !== QURAN_ROW_COUNT) {
    throw new Error(`[quran-stage:${sourceId}] row count ${count} != ${QURAN_ROW_COUNT}`);
  }
  if (rows.length !== QURAN_ROW_COUNT) {
    throw new Error(
      `[quran-stage:${sourceId}] coordinate count ${rows.length} != ${QURAN_ROW_COUNT}`,
    );
  }
  let previous = 0;
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]!;
    if (row.surah < 1 || row.surah > 114 || row.ayah < 1) {
      throw new Error(
        `[quran-stage:${sourceId}] bad coordinate ${row.globalIndex}/${row.surah}:${row.ayah}`,
      );
    }
    if (i === 0) previous = row.globalIndex;
    else if (row.globalIndex !== previous + 1) {
      throw new Error(
        `[quran-stage:${sourceId}] non-contiguous globalIndex at ${row.globalIndex}`,
      );
    }
    previous = row.globalIndex;
  }
  if (isArabicSourceId(sourceId)) {
    const profile = resolveSourceProfile(sourceId);
    if (profile.canonicalRowCount !== QURAN_ROW_COUNT) {
      throw new Error(`[quran-stage:${sourceId}] profile row count mismatch`);
    }
  }
}

function assertStagedQuranBytes(bytes: Uint8Array, sourceId: string): void {
  if (!sqlite3) throw new Error("[quran-worker] sqlite3 not initialized before staging check");
  const database = openReadOnly(bytes);
  try {
    const runner = createWasmQueryRunner(database);
    const count = runOne(runner, TANZIL_QURAN_DATABASE.queries.count);
    const rows = runQuery(runner, TANZIL_QURAN_DATABASE.queries.coordinates);
    assertStagedQuranContent(count, rows, sourceId);
  } finally {
    database.close();
  }
}

function stagedQuranValidator(): StagedValidator {
  return (bytes, spec) => assertStagedQuranBytes(bytes, spec.id);
}

async function bootArabic(
  manifest: ResolvedManifest,
  coordinates: CanonicalQuranCoordinates,
): Promise<void> {
  status("init");
  sqlite3 = await init();

  const persistentSources = new Set([
    DEFAULT_QURAN_SOURCE_PLAN.reader,
    DEFAULT_QURAN_SOURCE_PLAN.search.display,
  ]);
  for (const sourceId of plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN)) {
    const spec = manifest.scripts.find((artifact) => artifact.id === sourceId);
    if (!spec) throw new Error(`manifest missing Quran source ${sourceId}`);
    const profile = resolveSourceProfile(spec.id);
    status("downloading", sourceId);
    const artifact = await ensureArtifact(spec, progressEmitter(spec), {
      validate: stagedQuranValidator(),
    });
    const database = openReadOnly(artifact.bytes);
    const runner = createWasmQueryRunner(database);
    const source = loadQuranSource(runner, profile, coordinates);
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
  status("ready");
}

async function initialize(
  manifest: ResolvedManifest,
  coordinates: CanonicalQuranCoordinates,
  catalogue?: SourceCatalogueEntry[],
): Promise<void> {
  if (catalogue !== undefined) storedCatalogue = catalogue;
  if (ready) return;
  if (bootPromise !== null) {
    await bootPromise;
    return;
  }
  bootPromise = bootArabic(manifest, coordinates);
  try {
    await bootPromise;
    await refreshCachedTranslationIds();
  } finally {
    bootPromise = null;
  }
}

function readSurah(num: number, sourceId?: QuranSourceId): QuranSurahText {
  const state = sourceState(sourceId ?? DEFAULT_QURAN_SOURCE_PLAN.reader);
  if (!state.runner) throw new Error("reader source is not open");
  return {
    sourceId: state.source.profile.sourceId,
    script: state.source.profile.script,
    verses: readSourceSurah(state.runner, state.source, num),
    normalization: state.source.view.normalization(num),
  };
}

function rowsToRangeText(
  rows: ReadonlyArray<CanonicalQuranRow>,
  normalize: (surah: number) => SurahNormalization,
): QuranRangeText {
  return {
    ayahs: rows.map((row) => ({
      key: `${row.surah}:${row.ayah}`,
      surah: row.surah,
      ayah: row.ayah,
      globalIndex: row.globalIndex,
      text: row.text,
    })),
    normalizations: [...new Set(rows.map((row) => row.surah))].map(normalize),
  };
}

function readRange(from: number, to: number, sourceId?: QuranSourceId): QuranRangeText {
  const state = sourceState(sourceId ?? DEFAULT_QURAN_SOURCE_PLAN.reader);
  if (!state.runner) throw new Error("reader source is not open");
  return rowsToRangeText(readSourceRange(state.runner, state.source, from, to), (surah) =>
    state.source.view.normalization(surah),
  );
}

function translationSpec(sourceId: string): DownloadableSpec {
  if (storedCatalogue.length === 0) {
    throw new Error("translation catalogue unavailable");
  }
  const entry = storedCatalogue.find((candidate) =>
    candidate.kind === "translation"
      ? candidate.entry.id === sourceId
      : candidate.spec.id === sourceId,
  );
  if (!entry || entry.kind !== "translation") {
    throw new Error(`translation source ${sourceId} is not in the catalogue`);
  }
  const t = entry.entry;
  return Object.freeze({
    id: t.id,
    sizeBytes: t.sizeBytes,
    downloadUrl: t.downloadUrl,
  });
}

function evictTranslationDbs(): void {
  while (translationDbs.size >= TRANSLATION_DB_CAP) {
    const oldest = translationDbs.keys().next().value;
    if (oldest === undefined) break;
    const database = translationDbs.get(oldest);
    translationDbs.delete(oldest);
    if (database) {
      try {
        database.close();
      } catch {}
    }
  }
}

async function translationRunner(sourceId: string): Promise<QuranQueryRunner> {
  const cached = translationDbs.get(sourceId);
  if (cached) {
    translationDbs.delete(sourceId);
    translationDbs.set(sourceId, cached);
    return createWasmQueryRunner(cached);
  }
  const pending = pendingTranslationRunners.get(sourceId);
  if (pending) return pending;
  const run = fetchTranslationRunner(sourceId);
  pendingTranslationRunners.set(sourceId, run);
  try {
    return await run;
  } finally {
    pendingTranslationRunners.delete(sourceId);
  }
}

async function fetchTranslationRunner(sourceId: string): Promise<QuranQueryRunner> {
  const spec = translationSpec(sourceId);
  if (activeTranslationFetches++ === 0) status("downloading", sourceId);
  try {
    try {
      const artifact = await ensureArtifact(spec, progressEmitter(spec), {
        validate: stagedQuranValidator(),
      });
      if (artifact.downloaded) {
        void pruneTranslations({ pinnedArabicIds: PINNED_ARABIC, catalogue: storedCatalogue });
      }
      evictTranslationDbs();
      const database = openReadOnly(artifact.bytes);
      translationDbs.set(sourceId, database);
      return createWasmQueryRunner(database);
    } catch (error) {
      status("translation-fetch-failed", sourceId);
      throw error;
    }
  } finally {
    if (--activeTranslationFetches === 0) status("ready");
  }
}

function hasTranslationCached(sourceId: string): boolean {
  return translationDbs.has(sourceId) || cachedTranslationIds.has(sourceId);
}

async function refreshCachedTranslationIds(): Promise<void> {
  try {
    const artifacts = await listCachedArtifacts();
    cachedTranslationIds.clear();
    for (const a of artifacts) if (!isArabicSourceId(a.id)) cachedTranslationIds.add(a.id);
  } catch {}
}

async function ensureTranslation(sourceId: string): Promise<void> {
  if (hasTranslationCached(sourceId)) return;
  try {
    await translationRunner(sourceId);
    cachedTranslationIds.add(sourceId);
  } catch {}
}

function translationNormalization(sourceId: string, surah: number): SurahNormalization {
  return Object.freeze({
    surah,
    sourceId,
    script: QuranScript.Translation,
    sourceProfile: `translation:${sourceId}`,
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  });
}

async function readTranslationSurah(sourceId: string, num: number): Promise<QuranSurahText> {
  const runner = await translationRunner(sourceId);
  const verses = runQuery(runner, TANZIL_QURAN_DATABASE.queries.surah, [num]);
  return {
    sourceId,
    script: QuranScript.Translation,
    verses,
    normalization: translationNormalization(sourceId, num),
  };
}

async function readTranslationRange(
  sourceId: string,
  from: number,
  to: number,
): Promise<QuranRangeText> {
  const runner = await translationRunner(sourceId);
  return rowsToRangeText(
    runQuery(runner, TANZIL_QURAN_DATABASE.queries.range, [from, to]),
    (surah) => translationNormalization(sourceId, surah),
  );
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
  const matchRows = readAllRows(match);
  const displayRows = match === display ? matchRows : readAllRows(display);
  corpus = buildCanonicalSearchCorpus({
    matchRows,
    displayRows,
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

function runReaderOp<T>(
  source: QuranReaderSource | undefined,
  arabic: () => T,
  translation: (src: QuranReaderSource) => Promise<T> | T,
): Promise<T> | T {
  return source !== undefined && !isArabicSourceId(source) ? translation(source) : arabic();
}

type Handler<K extends WorkerRequest["type"]> = (
  msg: Extract<WorkerRequest, { type: K }>,
) => unknown;

const handlers: { [K in WorkerRequest["type"]]: Handler<K> } = {
  init: async (m) => {
    await initialize(m.manifest, m.coordinates, m.catalogue);
    return null;
  },
  refreshCatalogue: (m) => {
    if (m.catalogue !== undefined) storedCatalogue = m.catalogue;
    return null;
  },
  hasTranslation: (m) => hasTranslationCached(m.source),
  ensureTranslation: (m) => {
    void ensureTranslation(m.source);
    return null;
  },
  readSurah: (m) =>
    runReaderOp(
      m.source,
      () => readSurah(m.num, isArabicSourceId(m.source) ? m.source : undefined),
      (src) => readTranslationSurah(src, m.num),
    ),
  readRange: (m) =>
    runReaderOp(
      m.source,
      () => readRange(m.from, m.to, isArabicSourceId(m.source) ? m.source : undefined),
      (src) => readTranslationRange(src, m.from, m.to),
    ),
  search: (m) => search(m.query, m.opts),
};

ctx.onmessage = async (event: MessageEvent<WorkerRequest>): Promise<void> => {
  const message = event.data;
  const id = message.id;
  try {
    if (message.type === "init") {
      await handlers.init(message);
      emit({ id, ok: true, result: null });
      void pruneTranslations({ pinnedArabicIds: PINNED_ARABIC, catalogue: storedCatalogue });
      return;
    }
    if (!ready) {
      emit({ id, ok: false, error: "engine not ready" });
      return;
    }
    const handler = handlers[message.type] as (m: WorkerRequest) => unknown;
    emit({ id, ok: true, result: await handler(message) });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    if (message.type === "init") {
      status("error", errorMessage);
      emit({ type: "fatal", error: errorMessage });
    }
    emit({ id, ok: false, error: errorMessage });
  }
};
