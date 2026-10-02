import { RANGE_COUNTS, RangeKind, type QuranData } from "$lib/data/quran-data";
import type { RangeEntry } from "$lib/data/quran-types";

/**
 * Mushaf division math over the baked metadata JSON only — no Quran DB access,
 * no hashing. Juz, hizb and rub' all derive from the baked Page and HizbQuarter
 * delta series; a hizb is exactly four consecutive hizb-quarters.
 */
export const RUB_COUNT = RANGE_COUNTS[RangeKind.HizbQuarter];
const QUARTERS_PER_HIZB = 4;
export const HIZB_COUNT = RUB_COUNT / QUARTERS_PER_HIZB;

if (!Number.isSafeInteger(HIZB_COUNT) || HIZB_COUNT <= 0) {
  throw new Error("[mushaf-divisions] hizb quarter count must tile whole hizbs");
}

function rangeContaining(
  quranData: QuranData,
  kind: RangeKind,
  global: number,
): RangeEntry | undefined {
  return quranData
    .ranges(kind)
    .find((range) => global >= range.startGlobal && global <= range.endGlobal);
}

/** Global mushaf page (1..604) that contains the global ayah index. */
export function pageOfGlobal(quranData: QuranData, global: number): number {
  return rangeContaining(quranData, RangeKind.Page, global)?.index ?? 1;
}

/** Juz (1..30) that contains the global ayah index. */
export function juzOfPage(quranData: QuranData, global: number): number {
  return rangeContaining(quranData, RangeKind.Juz, global)?.index ?? 1;
}

/** Hizb-quarter (1..240) that contains the global ayah index. */
export function quarterOfPage(quranData: QuranData, global: number): number {
  return rangeContaining(quranData, RangeKind.HizbQuarter, global)?.index ?? 1;
}

/** Hizb (1..60) that contains the global ayah index: ceil(quarter / 4). */
export function hizbOfPage(quranData: QuranData, global: number): number {
  const quarter = quarterOfPage(quranData, global);
  return Math.ceil(quarter / QUARTERS_PER_HIZB);
}

/** Where the reader sits in mushaf divisions, for the sticky position indicator. */
export interface ReaderPositionState {
  globalPage: number;
  juz: number | null;
  hizb: number | null;
}

/** Full division position for one global ayah index. */
export function positionForGlobal(quranData: QuranData, global: number): ReaderPositionState {
  return {
    globalPage: pageOfGlobal(quranData, global),
    juz: juzOfPage(quranData, global),
    hizb: hizbOfPage(quranData, global),
  };
}

/**
 * Hizb `hizb` as one continuous range: the union of quarters 4h-3..4h. Same
 * RangeEntry shape the juz/page families produce, so the range loaders render
 * it unmodified.
 */
export function hizbRange(quranData: QuranData, hizb: number): RangeEntry | undefined {
  if (!Number.isSafeInteger(hizb) || hizb < 1 || hizb > HIZB_COUNT) return undefined;
  const first = quranData.rangeByIndex(RangeKind.HizbQuarter, hizb * QUARTERS_PER_HIZB - 3);
  const last = quranData.rangeByIndex(RangeKind.HizbQuarter, hizb * QUARTERS_PER_HIZB);
  if (!first || !last) return undefined;
  return {
    index: hizb,
    startGlobal: first.startGlobal,
    endGlobal: last.endGlobal,
    first: first.first,
    last: last.last,
  };
}
