/* ════════════════════════════════════════════════════════════════════════
   normalize.ts — the shared Arabic search normalization specification.

   One rule set applied to BOTH corpus-building and query-normalizing, so the
   offline Worker's match set is identical to the (future) live API's. This is
   the spec the Rust backend must implement byte-for-byte; parity is enforced by
   the shared fixture suite (__fixtures__/queries.json). Changing any rule bumps
   QURAN.searchVersion and ships updated fixtures.

   Rules (docs/quran-api.md §7.1):
     1. remove combining marks (Mn/Me) + format chars (Cf) — the harakat;
     2. remove tatweel (U+0640) and fold alef-wasla (U+0671) → bare alef;
     3. fold hamza-bearing alefs (آ أ إ) → bare alef (ا);
     4. fold alef-maqsura (ى) → ya (ي);
     5. fold ta-marbuta (ة) → ha (ه)  — §11.1 decision: improves recall, pinned;
     6. collapse whitespace runs to a single space and trim.

   Match semantics: substring of the normalized query within the normalized
   ayah. A query with spaces is one phrase. Results in ascending globalIndex.

   This module is imported by the Worker, so it must stay free of any
   SvelteKit/$env/$lib-config imports — it owns SEARCH_VERSION as the single
   source (site.ts re-exports it).
   ════════════════════════════════════════════════════════════════════════ */

/** The frozen normalization rule set. Bump when any rule above changes. */
export const SEARCH_VERSION = "arabic-search-v2";
export const MIN_QUERY_LEN = 3;
export const MAX_QUERY_LEN = 64;
export const DEFAULT_LIMIT = 20;
export const MAX_LIMIT = 50;
export const DEFAULT_OFFSET = 0;
export const MAX_OFFSET = 500;

const BARE_ALEF = "ا";
const YA = "ي";
const HA = "ه";
const REMOVED = /[\p{Mn}\p{Me}\p{Cf}\u0640]/u;

export interface NormalizedArabicMap {
  normalized: string;
  /** UTF-16 offsets into the original string, indexed by normalized code unit. */
  starts: number[];
  ends: number[];
}

/** Normalize Arabic while retaining positions into the unmodified input. */
export function normalizeArabicWithMap(input: string): NormalizedArabicMap {
  const values: { value: string; start: number; end: number }[] = [];
  let utf16 = 0;
  for (const scalar of input) {
    const start = utf16;
    utf16 += scalar.length;
    if (REMOVED.test(scalar)) {
      const previous = values.at(-1);
      if (previous) previous.end = utf16;
      continue;
    }

    let value = scalar;
    if (/[آأإٱ]/u.test(value)) value = BARE_ALEF;
    else if (value === "ى") value = YA;
    else if (value === "ة") value = HA;

    if (/\s/u.test(value)) {
      if (values.length === 0) continue;
      if (values.at(-1)?.value === " ") {
        values.at(-1)!.end = utf16;
        continue;
      }
      value = " ";
    }
    values.push({ value, start, end: utf16 });
  }
  if (values.at(-1)?.value === " ") values.pop();

  const starts: number[] = [];
  const ends: number[] = [];
  let normalized = "";
  for (const value of values) {
    normalized += value.value;
    for (let i = 0; i < value.value.length; i++) {
      starts.push(value.start);
      ends.push(value.end);
    }
  }
  return { normalized, starts, ends };
}

/** Normalize an Arabic search value (corpus or query) per §7.1. */
export function normalizeArabic(input: string): string {
  return normalizeArabicWithMap(input).normalized;
}

/** Unicode scalar count (surrogate pairs counted as one). */
export function scalarLength(s: string): number {
  let n = 0;
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  for (const _ of s) n++;
  return n;
}

/** A query is eligible after normalization when its scalar length is in range. */
export function isEligibleQuery(norm: string): boolean {
  const len = scalarLength(norm);
  return len >= MIN_QUERY_LEN && len <= MAX_QUERY_LEN;
}
