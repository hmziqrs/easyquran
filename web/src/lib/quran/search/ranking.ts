/**
 * Relevance scoring for translation hits (search-system §5). Matching stays a
 * contiguous `String.includes` find — ranking only reorders the matches:
 *
 * - term frequency — every occurrence of the query phrase in a verse counts;
 * - exact-phrase boost — a phrase bounded by non-word characters (whole words,
 *   not a fragment inside a longer word) outranks a partial-word hit;
 * - early-verse bias — a fraction of a point toward lower mushaf positions,
 *   small enough that a single extra occurrence always outweighs it.
 *
 * Equal scores fall back to mushaf (globalIndex) order, so ties keep the order
 * mushaf order would have given.
 */

const TERM_FREQUENCY_WEIGHT = 3;
const EXACT_PHRASE_WEIGHT = 2;
const EARLY_VERSE_WEIGHT = 0.5;

const WORD_CHAR = /[\p{L}\p{N}_]/u;

interface RankableUnit {
  readonly norm: string;
  readonly globalIndex: number;
}

function isWordChar(ch: string | undefined): boolean {
  return ch !== undefined && WORD_CHAR.test(ch);
}

/**
 * Occurrence count mirrors the highlight loop in `translation-corpus.ts`, so
 * `highlights.length` always equals the counted term frequency.
 */
function scoreUnit(unit: RankableUnit, normalizedQuery: string, corpusSize: number): number {
  let occurrences = 0;
  let exactPhrase = false;
  let from = 0;
  const lastStart = unit.norm.length - normalizedQuery.length;
  while (from <= lastStart) {
    const start = unit.norm.indexOf(normalizedQuery, from);
    if (start < 0) break;
    occurrences += 1;
    const end = start + normalizedQuery.length;
    const before = start > 0 ? unit.norm[start - 1] : undefined;
    if (!isWordChar(before) && !isWordChar(unit.norm[end])) exactPhrase = true;
    from = start + Math.max(1, normalizedQuery.length);
  }
  const earlyVerse = corpusSize > 0 ? (corpusSize - unit.globalIndex) / corpusSize : 0;
  return (
    TERM_FREQUENCY_WEIGHT * occurrences +
    (exactPhrase ? EXACT_PHRASE_WEIGHT : 0) +
    EARLY_VERSE_WEIGHT * earlyVerse
  );
}

/** Ranks mushaf-ordered matches by relevance; equal scores keep mushaf order. */
export function rankTranslationUnits<T extends RankableUnit>(
  matches: readonly T[],
  normalizedQuery: string,
  corpusSize: number,
): T[] {
  if (normalizedQuery.length === 0) return [...matches];
  const scored = matches.map((unit) => ({
    unit,
    score: scoreUnit(unit, normalizedQuery, corpusSize),
  }));
  scored.sort((a, b) => b.score - a.score || a.unit.globalIndex - b.unit.globalIndex);
  return scored.map((entry) => entry.unit);
}
