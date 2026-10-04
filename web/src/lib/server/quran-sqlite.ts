import { existsSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

import {
  type QuranSourceId as QuranSourceIdValue,
  type QuranRangeText,
  type QuranSurahText,
} from "$lib/data/quran-types";
import { DEFAULT_QURAN_SOURCE_PLAN, plannedSourceIds } from "$lib/quran/source-plan";
import type { QuranQueryRunner } from "$lib/quran/sql";
import {
  resolveSourceProfile,
  sourceProfile,
  type QuranSourceProfile,
} from "$lib/quran/view/source-profiles";
import {
  loadQuranSource,
  readSourceRange,
  readSourceSurah,
  type LoadedQuranSource,
} from "$lib/quran/view/source-runtime";
import { QURAN_DATA } from "$lib/server/quran-data";

import { createNodeQueryRunner } from "./quran-node-query-runner";

function findSourcePath(profile: QuranSourceProfile): string {
  const candidates = [
    path.resolve(process.cwd(), profile.artifact.repositoryPath),
    path.resolve(process.cwd(), "..", profile.artifact.repositoryPath),
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? candidates[1]!;
}

interface SourceState {
  readonly database: DatabaseSync;
  readonly runner: QuranQueryRunner;
  readonly source: LoadedQuranSource;
}

const sourceCache = new Map<QuranSourceIdValue, SourceState>();

// Fail-soft reader seat, mirroring the worker's bootArabic demotion
// (web/src/lib/workers/quran.worker.ts): when the preferred annotated reader DB
// is not on disk, reads that name it (every default-source read) serve the plan's
// remaining pinned source with a one-time warning, so SSR and the Arabic prerender
// still complete on machines without the annotated artifact. The missing-DB throw
// survives only when the fallback DB is missing too.
let warnedReaderFallback = false;

function warnReaderFallbackOnce(preferred: QuranSourceIdValue, fallback: QuranSourceIdValue): void {
  if (warnedReaderFallback) return;
  warnedReaderFallback = true;
  const missing = sourceProfile(preferred).artifact.repositoryPath;
  console.warn(
    `[quran-sqlite] reader source ${preferred} unavailable (DB missing at ${missing}); serving fallback ${fallback} for default-source reads`,
  );
}

function resolveReaderSeat(sourceId: QuranSourceIdValue): QuranSourceIdValue {
  if (sourceId !== DEFAULT_QURAN_SOURCE_PLAN.reader) return sourceId;
  const preferredPath = findSourcePath(sourceProfile(sourceId));
  if (existsSync(preferredPath)) return sourceId;
  const fallback = plannedSourceIds(DEFAULT_QURAN_SOURCE_PLAN).find(
    (id) => id !== sourceId && existsSync(findSourcePath(sourceProfile(id))),
  );
  if (!fallback) return sourceId;
  warnReaderFallbackOnce(sourceId, fallback);
  return fallback;
}

function openSource(sourceId: QuranSourceIdValue): SourceState {
  const cached = sourceCache.get(sourceId);
  if (cached) return cached;

  const seatedId = resolveReaderSeat(sourceId);
  const seated = sourceCache.get(seatedId);
  if (seated) return seated;

  const registered = sourceProfile(seatedId);
  const sourcePath = findSourcePath(registered);
  if (!existsSync(sourcePath)) {
    throw new Error(`[quran-sqlite] missing ${seatedId} DB at ${sourcePath}`);
  }
  const profile = resolveSourceProfile(seatedId);
  const database = new DatabaseSync(sourcePath);
  database.exec("PRAGMA query_only = ON");

  try {
    const runner = createNodeQueryRunner(database);
    const source = loadQuranSource(runner, profile, QURAN_DATA.coordinates);
    const state = Object.freeze({ database, runner, source });
    sourceCache.set(seatedId, state);
    return state;
  } catch (error) {
    database.close();
    throw error;
  }
}

export function readSurahText(
  num: number,
  sourceId: QuranSourceIdValue = DEFAULT_QURAN_SOURCE_PLAN.reader,
): QuranSurahText {
  const state = openSource(sourceId);
  return {
    // Report the served source, not the requested one: under the fail-soft seat
    // the text comes from the fallback DB (same shape as the worker's readSurah).
    sourceId: state.source.profile.sourceId,
    script: state.source.profile.script,
    verses: readSourceSurah(state.runner, state.source, num),
    normalization: state.source.view.normalization(num),
  };
}

export function readSurahVerses(
  num: number,
  sourceId: QuranSourceIdValue = DEFAULT_QURAN_SOURCE_PLAN.reader,
): string[] {
  return readSurahText(num, sourceId).verses;
}

export function readRangeText(
  from: number,
  to: number,
  sourceId: QuranSourceIdValue = DEFAULT_QURAN_SOURCE_PLAN.reader,
): QuranRangeText {
  const state = openSource(sourceId);
  const rows = readSourceRange(state.runner, state.source, from, to);
  const ayahs = rows.map((row) => ({
    key: `${row.surah}:${row.ayah}`,
    surah: row.surah,
    ayah: row.ayah,
    globalIndex: row.globalIndex,
    text: row.text,
  }));
  const represented = new Set(rows.map((row) => row.surah));
  const normalizations = [...represented].map((surah) => state.source.view.normalization(surah));
  return { ayahs, normalizations };
}
