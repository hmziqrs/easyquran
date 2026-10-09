import raw from "./surah-names.json";

/**
 * Diacritized surah display names, baked from Al Quran Cloud (see
 * `web/scripts/gen-surah-names.ts`). Tanzil's `quran-data.xml` names stay the
 * `arabicPlain` matching form; these are display-only and never used for search.
 */
export interface SurahNamesSource {
  readonly source: string;
  readonly sourceUrl: string;
  readonly retrieved: string;
  readonly license: string;
}

function fail(message: string): never {
  throw new Error(`[surah-names] ${message}`);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- sole raw JSON decode boundary for one field
function nonEmptyString(value: unknown, label: string): string {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- validates the untyped JSON field at its sole decode boundary
  if (typeof value !== "string" || value === "") fail(`${label} must be a non-empty string`);
  return value;
}

const NAMES = raw.names;
if (!Array.isArray(NAMES) || NAMES.length !== 114) {
  fail(`expected 114 names, got ${Array.isArray(NAMES) ? NAMES.length : 0}`);
}

export const SURAH_NAMES_SOURCE: SurahNamesSource = Object.freeze({
  source: nonEmptyString(raw.source, "source"),
  sourceUrl: nonEmptyString(raw.sourceUrl, "sourceUrl"),
  retrieved: nonEmptyString(raw.retrieved, "retrieved"),
  license: nonEmptyString(raw.license, "license"),
});

export const SURAH_NAMES_ARABIC: readonly string[] = Object.freeze(
  NAMES.map((name, index) => nonEmptyString(name, `name ${index + 1}`)),
);
