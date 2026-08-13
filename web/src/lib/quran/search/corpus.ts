import { clamp } from "es-toolkit";
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
import { alignSearchText } from "./alignment.ts";
import { SearchHitKind, type Highlight, type SearchHit, type SearchOpts } from "./types.ts";

interface SearchUnitBase {
  anchorGlobal: number;
  rank: 0 | 1;
  matchNorm: string;
  displayText: string;
  matchDisplayStarts: number[];
  matchDisplayEnds: number[];
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
        const alignment = alignSearchText(matchOpener.text, displayOpener.text);
        units.push({
          kind: SearchHitKind.Opener,
          surah: matchRow.surah,
          anchorGlobal: matchRow.globalIndex,
          rank: 0,
          ...alignment,
          displayText: displayOpener.text,
          displayBaseUtf16: 0,
        });
      }
    }

    const matchBody = input.matchView.body(matchRow.surah, matchRow.ayah, matchRow.text);
    const displayBody = input.displayView.body(displayRow.surah, displayRow.ayah, displayRow.text);
    const descriptor = input.displayView.normalization(displayRow.surah);
    const alignment = alignSearchText(matchBody, displayBody);
    units.push({
      kind: SearchHitKind.Ayah,
      surah: matchRow.surah,
      ayah: matchRow.ayah,
      globalIndex: matchRow.globalIndex,
      anchorGlobal: matchRow.globalIndex,
      rank: 1,
      ...alignment,
      displayText: displayRow.text,
      displayBaseUtf16:
        displayRow.ayah === 1 ? scalarToUtf16Index(displayRow.text, descriptor.bodyStartScalar) : 0,
    });
  }

  return units.sort((a, b) => a.anchorGlobal - b.anchorGlobal || a.rank - b.rank);
}

function highlightsFor(unit: CanonicalSearchUnit, normalizedQuery: string): Highlight[] {
  const highlights: Highlight[] = [];
  let from = 0;
  while (from <= unit.matchNorm.length - normalizedQuery.length) {
    const start = unit.matchNorm.indexOf(normalizedQuery, from);
    if (start < 0) break;
    const last = start + normalizedQuery.length - 1;
    const rawStart = unit.matchDisplayStarts[start];
    const rawEnd = unit.matchDisplayEnds[last];
    if (rawStart !== undefined && rawEnd !== undefined && rawStart < rawEnd) {
      highlights.push({
        start: unit.displayBaseUtf16 + rawStart,
        end: unit.displayBaseUtf16 + rawEnd,
      });
    }
    from = start + Math.max(1, normalizedQuery.length);
  }
  return highlights;
}

function hitFor(unit: CanonicalSearchUnit, normalizedQuery: string): SearchHit {
  const highlights = highlightsFor(unit, normalizedQuery);
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
  const limit = clamp(opts.limit ?? DEFAULT_LIMIT, 0, MAX_LIMIT);
  const offset = clamp(opts.offset ?? DEFAULT_OFFSET, 0, MAX_OFFSET);
  if (!isEligibleQuery(normalized)) return { total: 0, limit, offset, results: [] };

  const matching = units.filter((unit) => unit.matchNorm.includes(normalized));
  return {
    total: matching.length,
    limit,
    offset,
    results: matching.slice(offset, offset + limit).map((unit) => hitFor(unit, normalized)),
  };
}
