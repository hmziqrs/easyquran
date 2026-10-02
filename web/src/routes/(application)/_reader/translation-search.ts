/**
 * Token-based fuzzy search over translation catalogue rows.
 *
 * A row matches when EVERY whitespace-separated query token hits at least one
 * field (translation name, translator, language, language code, country,
 * native-script autonym). A token hits a field when any word of the field
 * contains the token, or sits within edit distance 1 of it (typo tolerance,
 * only for tokens of 4+ chars). Comparison is diacritic-insensitive (NFD,
 * combining marks stripped) and case-insensitive, so "Garcia" matches
 * "García" and "englsh" matches "English".
 */

/** Fields of a catalogue row the search matches against. */
export interface TranslationSearchFields {
  readonly name: string;
  readonly translator: string | null;
  readonly language: string;
  readonly languageCode: string;
  readonly country: string;
  /** Native-script autonym ("اردو" for Urdu); null when the catalogue has none. */
  readonly autonym: string | null;
}

/** lowercase + strip combining marks (NFD), so "García" → "garcia". */
function normalize(value: string): string {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{Mark}/gu, "");
}

function words(value: string): string[] {
  return normalize(value).split(/[\s'’-]+/).filter(Boolean);
}

/** Classic two-row Levenshtein distance; inputs here are short single words. */
function editDistance(a: string, b: string): number {
  if (Math.abs(a.length - b.length) > 1) return 2;
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i += 1) {
    const curr = [i];
    for (let j = 1; j <= b.length; j += 1) {
      const substitution = prev[j - 1]! + (a[i - 1] === b[j - 1] ? 0 : 1);
      curr[j] = Math.min(prev[j]! + 1, curr[j - 1]! + 1, substitution);
    }
    prev = curr;
  }
  return prev[b.length]!;
}

function tokenHitsWord(token: string, word: string): boolean {
  if (word.includes(token)) return true;
  if (token.length < 4) return false;
  return editDistance(token, word) <= 1;
}

function tokenHitsField(token: string, fieldWords: string[]): boolean {
  return fieldWords.some((word) => tokenHitsWord(token, word));
}

/** Does a single normalized query token hit any of the row's fields? */
export function translationTokenMatches(
  token: string,
  fields: TranslationSearchFields,
): boolean {
  const t = normalize(token);
  if (!t) return true;
  const fieldWords = [
    words(fields.name),
    fields.translator === null ? [] : words(fields.translator),
    words(fields.language),
    words(fields.languageCode),
    fields.country === "" ? [] : words(fields.country),
    fields.autonym === null ? [] : words(fields.autonym),
  ];
  return fieldWords.some((fw) => tokenHitsField(t, fw));
}

/** True when every query token hits some field of the row (empty query: true). */
export function translationMatchesQuery(
  query: string,
  fields: TranslationSearchFields,
): boolean {
  const tokens = normalize(query).split(/\s+/).filter(Boolean);
  return tokens.every((token) => translationTokenMatches(token, fields));
}
