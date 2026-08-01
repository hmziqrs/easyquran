import type { CanonicalQuranRow } from "../sql.ts";
import { OpenerKind } from "../../data/quran-types.ts";
import type { QuranSourceView } from "../view/source-view.ts";
import { scalarToUtf16Index } from "../view/source-view.ts";
import {
  DEFAULT_LIMIT,
  DEFAULT_OFFSET,
  MAX_LIMIT,
  MAX_OFFSET,
  isEligibleQuery,
  normalizeArabic,
} from "./normalize.ts";
import { SearchHitKind, type Highlight, type SearchHit, type SearchOpts } from "./types.ts";

interface SearchUnitBase {
  anchorGlobal: number;
  rank: 0 | 1;
  matchNorm: string;
  displayText: string;
  displayMatchText: string;
  displayBaseUtf16: number;
}

export type CanonicalSearchUnit =
  | (SearchUnitBase & { kind: typeof SearchHitKind.Opener; surah: number })
  | (SearchUnitBase & {
      kind: typeof SearchHitKind.Ayah;
      surah: number;
      ayah: number;
      globalIndex: number;
    });

/**
 * Build source-independent search units: numbered bodies plus distinct opener
 * headers. Matching and display sources may use different orthographies.
 */
export function buildCanonicalSearchCorpus(input: {
  matchRows: readonly CanonicalQuranRow[];
  displayRows: readonly CanonicalQuranRow[];
  matchView: QuranSourceView;
  displayView: QuranSourceView;
}): CanonicalSearchUnit[] {
  const displayByIndex = new Map(input.displayRows.map((row) => [row.globalIndex, row]));
  const units: CanonicalSearchUnit[] = [];

  for (const matchRow of input.matchRows) {
    const displayRow = displayByIndex.get(matchRow.globalIndex);
    if (!displayRow || displayRow.surah !== matchRow.surah || displayRow.ayah !== matchRow.ayah) {
      throw new Error(`[quran-search] source coordinate mismatch at ${matchRow.globalIndex}`);
    }

    if (matchRow.ayah === 1) {
      const matchOpener = input.matchView.opener(matchRow.surah);
      const displayOpener = input.displayView.opener(matchRow.surah);
      if (matchOpener.kind === OpenerKind.Header) {
        if (!matchOpener.text || displayOpener.kind !== OpenerKind.Header || !displayOpener.text) {
          throw new Error(`[quran-search] missing opener text for surah ${matchRow.surah}`);
        }
        units.push({
          kind: SearchHitKind.Opener,
          surah: matchRow.surah,
          anchorGlobal: matchRow.globalIndex,
          rank: 0,
          matchNorm: normalizeArabic(matchOpener.text),
          displayText: displayOpener.text,
          displayMatchText: displayOpener.text,
          displayBaseUtf16: 0,
        });
      }
    }

    const matchBody = input.matchView.body(matchRow.surah, matchRow.ayah, matchRow.text);
    const displayBody = input.displayView.body(displayRow.surah, displayRow.ayah, displayRow.text);
    const descriptor = input.displayView.normalization(displayRow.surah);
    units.push({
      kind: SearchHitKind.Ayah,
      surah: matchRow.surah,
      ayah: matchRow.ayah,
      globalIndex: matchRow.globalIndex,
      anchorGlobal: matchRow.globalIndex,
      rank: 1,
      matchNorm: normalizeArabic(matchBody),
      displayText: displayRow.text,
      displayMatchText: displayBody,
      displayBaseUtf16:
        displayRow.ayah === 1 ? scalarToUtf16Index(displayRow.text, descriptor.bodyStartScalar) : 0,
    });
  }

  return units.sort((a, b) => a.anchorGlobal - b.anchorGlobal || a.rank - b.rank);
}

interface NormalizedMap {
  normalized: string;
  starts: number[];
  ends: number[];
}

const REMOVED = /[\p{Mn}\p{Me}\p{Cf}\u0640]/u;

/** Normalize while retaining UTF-16 positions into the original display text. */
function normalizeWithMap(input: string): NormalizedMap {
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
    if (/[آأإٱ]/u.test(value)) value = "ا";
    else if (value === "ى") value = "ي";
    else if (value === "ة") value = "ه";

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

function highlightsFor(text: string, normalizedQuery: string, baseUtf16: number): Highlight[] {
  const mapped = normalizeWithMap(text);
  const highlights: Highlight[] = [];
  let from = 0;
  while (from <= mapped.normalized.length - normalizedQuery.length) {
    const start = mapped.normalized.indexOf(normalizedQuery, from);
    if (start < 0) break;
    const last = start + normalizedQuery.length - 1;
    const rawStart = mapped.starts[start];
    const rawEnd = mapped.ends[last];
    if (rawStart !== undefined && rawEnd !== undefined) {
      highlights.push({ start: baseUtf16 + rawStart, end: baseUtf16 + rawEnd });
    }
    from = start + Math.max(1, normalizedQuery.length);
  }
  return highlights;
}

function hitFor(unit: CanonicalSearchUnit, normalizedQuery: string): SearchHit {
  const highlights = highlightsFor(unit.displayMatchText, normalizedQuery, unit.displayBaseUtf16);
  if (unit.kind === SearchHitKind.Opener) {
    return {
      kind: SearchHitKind.Opener,
      key: `opener:${unit.surah}`,
      surah: unit.surah,
      anchorAyah: 1,
      text: unit.displayText,
      highlights,
    };
  }
  return {
    kind: SearchHitKind.Ayah,
    ayah: {
      key: `${unit.surah}:${unit.ayah}`,
      surah: unit.surah,
      ayah: unit.ayah,
      globalIndex: unit.globalIndex,
      text: unit.displayText,
    },
    highlights,
  };
}

export function searchCanonicalCorpus(
  units: readonly CanonicalSearchUnit[],
  query: string,
  opts: SearchOpts = {},
): { total: number; limit: number; offset: number; results: SearchHit[] } {
  const normalized = normalizeArabic(query);
  const limit = Math.max(0, Math.min(opts.limit ?? DEFAULT_LIMIT, MAX_LIMIT));
  const offset = Math.max(0, Math.min(opts.offset ?? DEFAULT_OFFSET, MAX_OFFSET));
  if (!isEligibleQuery(normalized)) return { total: 0, limit, offset, results: [] };

  const matching = units.filter((unit) => unit.matchNorm.includes(normalized));
  return {
    total: matching.length,
    limit,
    offset,
    results: matching.slice(offset, offset + limit).map((unit) => hitFor(unit, normalized)),
  };
}
