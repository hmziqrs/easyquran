import { existsSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  type QuranSourceId as QuranSourceIdValue,
  type QuranRangeText,
  type QuranSurahText,
} from "$lib/data/quran-types";
import type { QuranQueryRunner } from "$lib/quran/sql";
import { QURAN_DATA } from "$lib/server/quran-data";
import { DEFAULT_QURAN_SOURCE_PLAN } from "$lib/quran/source-plan";
import {
  loadQuranSource,
  readSourceRange,
  readSourceSurah,
  type LoadedQuranSource,
} from "$lib/quran/view/source-runtime";
import {
  resolveSourceProfile,
  sourceProfile,
  type QuranSourceProfile,
} from "$lib/quran/view/source-profiles";
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

function openSource(sourceId: QuranSourceIdValue): SourceState {
  const cached = sourceCache.get(sourceId);
  if (cached) return cached;

  const registered = sourceProfile(sourceId);
  const sourcePath = findSourcePath(registered);
  if (!existsSync(sourcePath)) {
    throw new Error(`[quran-sqlite] missing ${sourceId} DB at ${sourcePath}`);
  }
  const profile = resolveSourceProfile(sourceId);
  const database = new DatabaseSync(sourcePath);
  database.exec("PRAGMA query_only = ON");

  try {
    const runner = createNodeQueryRunner(database);
    const source = loadQuranSource(runner, profile, QURAN_DATA.coordinates);
    const state = Object.freeze({ database, runner, source });
    sourceCache.set(sourceId, state);
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
    sourceId,
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
