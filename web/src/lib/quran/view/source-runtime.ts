import { runOne, runQuery, type CanonicalQuranRow, type QuranQueryRunner } from "../sql.ts";
import {
  CANONICAL_QURAN_COORDINATES,
  validateCanonicalCoordinates,
} from "./canonical-coordinates.ts";
import type { QuranSourceProfile } from "./source-profiles.ts";
import { createQuranSourceView, type QuranSourceView } from "./source-view.ts";

export interface LoadedQuranSource {
  readonly profile: QuranSourceProfile;
  readonly view: QuranSourceView;
}

export function loadQuranSource(
  runner: QuranQueryRunner,
  profile: QuranSourceProfile,
): LoadedQuranSource {
  const rowCount = runOne(runner, profile.database.queries.count);
  if (rowCount !== profile.canonicalRowCount) {
    throw new Error(
      `[quran-source:${profile.id}] row count ${rowCount} != ${profile.canonicalRowCount}`,
    );
  }
  if (profile.canonicalRowCount !== CANONICAL_QURAN_COORDINATES.rowCount) {
    throw new Error(
      `[quran-source:${profile.id}] profile row count ${profile.canonicalRowCount} != canonical ${CANONICAL_QURAN_COORDINATES.rowCount}`,
    );
  }
  validateCanonicalCoordinates(profile.id, runQuery(runner, profile.database.queries.coordinates));
  const firstAyahs = runQuery(runner, profile.database.queries.firstAyahs);
  const openerRows = profile.database.queries.openers
    ? runQuery(runner, profile.database.queries.openers)
    : [];
  const openerTexts = new Map<number, string>();
  for (const row of openerRows) {
    if (openerTexts.has(row.surah)) {
      throw new Error(`[quran-source:${profile.id}] duplicate opener for surah ${row.surah}`);
    }
    openerTexts.set(row.surah, row.text);
  }
  return Object.freeze({
    profile,
    view: createQuranSourceView({ profile, firstAyahs, openerTexts }),
  });
}

export function readSourceSurah(
  runner: QuranQueryRunner,
  source: LoadedQuranSource,
  surah: number,
): string[] {
  return runQuery(runner, source.profile.database.queries.surah, [surah]);
}

export function readSourceRange(
  runner: QuranQueryRunner,
  source: LoadedQuranSource,
  from: number,
  to: number,
): CanonicalQuranRow[] {
  return runQuery(runner, source.profile.database.queries.range, [from, to]);
}

export function readAllSourceRows(
  runner: QuranQueryRunner,
  source: LoadedQuranSource,
): CanonicalQuranRow[] {
  return runQuery(runner, source.profile.database.queries.all);
}
