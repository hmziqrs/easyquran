/* Typed, platform-neutral Quran SQL adapter contract.

   Node SSG and sqlite-wasm implement only QuranQueryRunner. Query text, row
   decoding, validation, and canonical field names live here once. A source
   with a different schema supplies another QuranDatabaseAdapter. */

export type SqlValue = string | number | null | Uint8Array;
export type SqlRow = Readonly<Record<string, unknown>>;

export interface QuranQueryRunner {
  all(sql: string, params?: readonly SqlValue[]): SqlRow[];
}

export interface QuranQuery<Result> {
  readonly sql: string;
  readonly decode: (row: SqlRow) => Result;
}

export interface CanonicalQuranRow {
  globalIndex: number;
  surah: number;
  ayah: number;
  text: string;
}

/** Source coordinates without text, used for exhaustive load-time validation. */
export type QuranCoordinateRow = Omit<CanonicalQuranRow, "text">;

export interface FirstAyahRow {
  surah: number;
  text: string;
}

export interface QuranSourceQueries {
  readonly count: QuranQuery<number>;
  readonly coordinates: QuranQuery<QuranCoordinateRow>;
  readonly firstAyahs: QuranQuery<FirstAyahRow>;
  /** Exact unnumbered opener text for chapter-flag or separate-row sources. */
  readonly openers?: QuranQuery<FirstAyahRow>;
  readonly surah: QuranQuery<string>;
  readonly range: QuranQuery<CanonicalQuranRow>;
  readonly all: QuranQuery<CanonicalQuranRow>;
}

export const QuranDatabaseAdapterId = {
  TanzilQuranTextV1: "tanzil-quran-text-v1",
} as const;
export type QuranDatabaseAdapterId =
  (typeof QuranDatabaseAdapterId)[keyof typeof QuranDatabaseAdapterId];

export interface QuranDatabaseAdapter {
  readonly id: string;
  readonly queries: QuranSourceQueries;
}

export function defineQuranQuery<Result>(
  sql: string,
  decode: (row: SqlRow) => Result,
): QuranQuery<Result> {
  return Object.freeze({ sql, decode });
}

export function decodeIntegerField(row: SqlRow, key: string): number {
  const value = row[key];
  if (typeof value !== "number" || !Number.isSafeInteger(value)) {
    throw new Error(`[quran-sql] ${key} must be a safe integer`);
  }
  return value;
}

export function decodeTextField(row: SqlRow, key = "text"): string {
  const value = row[key];
  if (typeof value !== "string") throw new Error(`[quran-sql] ${key} must be a string`);
  return value;
}

export function decodeCanonicalRow(row: SqlRow): CanonicalQuranRow {
  return {
    globalIndex: decodeIntegerField(row, "globalIndex"),
    surah: decodeIntegerField(row, "surah"),
    ayah: decodeIntegerField(row, "ayah"),
    text: decodeTextField(row),
  };
}

export function decodeCoordinateRow(row: SqlRow): QuranCoordinateRow {
  return {
    globalIndex: decodeIntegerField(row, "globalIndex"),
    surah: decodeIntegerField(row, "surah"),
    ayah: decodeIntegerField(row, "ayah"),
  };
}

export function defineQuranDatabaseAdapter(adapter: QuranDatabaseAdapter): QuranDatabaseAdapter {
  return Object.freeze(adapter);
}

/** Execute and decode a query identically in every runtime. */
export function runQuery<Result>(
  runner: QuranQueryRunner,
  query: QuranQuery<Result>,
  params: readonly SqlValue[] = [],
): Result[] {
  return runner.all(query.sql, params).map((row) => query.decode(row));
}

/** Require exactly one decoded row. */
export function runOne<Result>(
  runner: QuranQueryRunner,
  query: QuranQuery<Result>,
  params: readonly SqlValue[] = [],
): Result {
  const rows = runQuery(runner, query, params);
  if (rows.length !== 1) {
    throw new Error(`[quran-sql] expected one row, received ${rows.length}`);
  }
  return rows[0]!;
}

/** Tanzil quran_text schema projected into the canonical row model. */
export const TANZIL_QURAN_DATABASE = defineQuranDatabaseAdapter({
  id: QuranDatabaseAdapterId.TanzilQuranTextV1,
  queries: Object.freeze({
    count: defineQuranQuery("SELECT count(*) AS count FROM quran_text", (row) =>
      decodeIntegerField(row, "count"),
    ),
    coordinates: defineQuranQuery(
      'SELECT "index" AS globalIndex, sura AS surah, aya AS ayah FROM quran_text ORDER BY "index"',
      decodeCoordinateRow,
    ),
    firstAyahs: defineQuranQuery(
      "SELECT sura AS surah, text FROM quran_text WHERE aya = 1 ORDER BY sura",
      (row) => ({ surah: decodeIntegerField(row, "surah"), text: decodeTextField(row) }),
    ),
    surah: defineQuranQuery(
      "SELECT text FROM quran_text WHERE sura = ? ORDER BY aya",
      decodeTextField,
    ),
    range: defineQuranQuery(
      'SELECT "index" AS globalIndex, sura AS surah, aya AS ayah, text FROM quran_text WHERE "index" BETWEEN ? AND ? ORDER BY "index"',
      decodeCanonicalRow,
    ),
    all: defineQuranQuery(
      'SELECT "index" AS globalIndex, sura AS surah, aya AS ayah, text FROM quran_text ORDER BY "index"',
      decodeCanonicalRow,
    ),
  }),
});
