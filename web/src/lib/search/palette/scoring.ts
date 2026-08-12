import { normalizeArabic } from "$lib/quran/search/normalize";

function isSubsequence(haystack: string, needle: string): boolean {
  let i = 0;
  for (const ch of haystack) {
    if (i === needle.length) return true;
    if (ch === needle[i]) i += 1;
  }
  return i === needle.length;
}

/**
 * Cheap, deterministic relevance in `[0, 1]`, shared by every source so groups
 * rank comparably. Exact beats prefix beats word-start beats substring beats
 * subsequence; `0` means no match, so callers can filter on truthiness.
 */
export function scoreText(haystack: string, needle: string): number {
  const hay = haystack.trim().toLowerCase();
  const q = needle.trim().toLowerCase();
  if (hay.length === 0 || q.length === 0) return 0;
  if (hay === q) return 1;
  if (hay.startsWith(q)) return 0.9;
  const at = hay.indexOf(q);
  if (at > 0) return /[\s\-'’.]/.test(hay[at - 1]!) ? 0.8 : 0.6;
  return isSubsequence(hay, q) ? 0.3 : 0;
}

/** Best score across several fields — name, transliteration, meaning, slug… */
export function scoreFields(fields: readonly (string | undefined)[], needle: string): number {
  let best = 0;
  for (const field of fields) {
    if (!field) continue;
    const score = scoreText(field, needle);
    if (score > best) best = score;
  }
  return best;
}

/**
 * Arabic matching over the same normalization the Quran text search uses, so
 * `البقرة` matches regardless of harakat, alef form or tatweel.
 */
export function scoreArabic(haystack: string, normalizedNeedle: string): number {
  if (normalizedNeedle.length === 0) return 0;
  const hay = normalizeArabic(haystack);
  if (hay.length === 0) return 0;
  if (hay === normalizedNeedle) return 1;
  if (hay.startsWith(normalizedNeedle)) return 0.9;
  return hay.includes(normalizedNeedle) ? 0.6 : 0;
}

/** Sorts by score descending, then by a caller-supplied stable tiebreak. */
export function byScore<T extends { score: number }>(
  entries: T[],
  tiebreak: (a: T, b: T) => number = () => 0,
): T[] {
  return entries.sort((a, b) => b.score - a.score || tiebreak(a, b));
}
