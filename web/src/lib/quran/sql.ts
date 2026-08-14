export type SqlValue = string | number | null | Uint8Array;
export type SqlRow = Readonly<Record<string, SqlValue>>;

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

export type QuranCoordinateRow = Omit<CanonicalQuranRow, "text">;

export interface FirstAyahRow {
  surah: number;
  text: string;
}

export interface QuranSourceQueries {
  readonly count: QuranQuery<number>;
  readonly coordinates: QuranQuery<QuranCoordinateRow>;
  readonly firstAyahs: QuranQuery<FirstAyahRow>;
  readonly openers?: QuranQuery<FirstAyahRow>;
  readonly surah: QuranQuery<string>;
  readonly range: QuranQuery<CanonicalQuranRow>;
  readonly all: QuranQuery<CanonicalQuranRow>;
}

export const QuranDatabaseAdapterId = {
  QuranTextV1: "quran-text-v1",
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
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- sqlite cell arrives as the SqlValue union; typeof IS the field parse at the DB boundary
  if (typeof value !== "number" || !Number.isSafeInteger(value)) {
    throw new Error(`[quran-sql] ${key} must be a safe integer`);
  }
  return value;
}

export function decodeTextField(row: SqlRow, key = "text"): string {
  const value = row[key];
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- sqlite cell arrives as the SqlValue union; typeof IS the field parse at the DB boundary
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

export function runQuery<Result>(
  runner: QuranQueryRunner,
  query: QuranQuery<Result>,
  params: readonly SqlValue[] = [],
): Result[] {
  return runner.all(query.sql, params).map((row) => query.decode(row));
}

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

export const QURAN_TEXT_DATABASE = defineQuranDatabaseAdapter({
  id: QuranDatabaseAdapterId.QuranTextV1,
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
