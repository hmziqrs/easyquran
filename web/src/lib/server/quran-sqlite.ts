/* Server-only registered Quran source reader for SSG.

   This module owns filesystem and node:sqlite concerns only. Query execution,
   schema adaptation, row decoding, source validation, and canonical views are
   shared with sqlite-wasm through the platform-neutral source runtime. */

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  type Ayah,
  type QuranSourceId as QuranSourceIdValue,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";
import type { QuranQueryRunner } from "$lib/quran/sql";
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
  readonly sha256: string;
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
  const sha256 = createHash("sha256").update(readFileSync(sourcePath)).digest("hex");
  const profile = resolveSourceProfile(sourceId, sha256);
  const database = new DatabaseSync(sourcePath);
  database.exec("PRAGMA query_only = ON");

  try {
    const runner = createNodeQueryRunner(database);
    const source = loadQuranSource(runner, profile);
    const state = Object.freeze({ sha256, database, runner, source });
    sourceCache.set(sourceId, state);
    return state;
  } catch (error) {
    database.close();
    throw error;
  }
}

/** Raw verses plus the serializable canonical descriptor for one surah. */
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

/** Back-compatible raw-only accessor for gallery specimens. */
export function readSurahVerses(
  num: number,
  sourceId: QuranSourceIdValue = DEFAULT_QURAN_SOURCE_PLAN.reader,
): string[] {
  return readSurahText(num, sourceId).verses;
}

/** Raw ayahs and source descriptors in an inclusive global range. */
export function readRangeText(
  from: number,
  to: number,
  sourceId: QuranSourceIdValue = DEFAULT_QURAN_SOURCE_PLAN.reader,
): { ayahs: Ayah[]; normalizations: SurahNormalization[] } {
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

export function validateSource(sourceId: QuranSourceIdValue): {
  rows: number;
  sha256: string;
  ok: boolean;
} {
  const state = openSource(sourceId);
  const profile = state.source.profile;
  return {
    rows: profile.canonicalRowCount,
    sha256: state.sha256,
    ok: state.sha256 === profile.artifact.sha256,
  };
}

export function validateReaderSource(): { rows: number; sha256: string; ok: boolean } {
  return validateSource(DEFAULT_QURAN_SOURCE_PLAN.reader);
}
