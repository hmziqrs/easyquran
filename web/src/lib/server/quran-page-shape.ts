import { RangeKind, SURAH_COUNT } from "$lib/data/quran-data";
import { hizbRange, positionForGlobal } from "$lib/data/mushaf-divisions";
import type {
  Ayah,
  CatalogEntry,
  MushafPageLink,
  RangeEntry,
  RangePageData,
  RangeRouteKind,
  SurahLink,
  SurahNormalization,
} from "$lib/data/quran-types";
import { QURAN_DATA, toSurahLink } from "$lib/server/quran-data";
// Shared shape builders for the SSR loaders (Arabic/translation × surah/range). They
// differ only in where the ayah text comes from; the navigation and range envelope around it are
// identical, so they live here once.
import { error } from "@sveltejs/kit";

export interface SurahRouteNav {
  previousPage: MushafPageLink | null;
  nextPage: MushafPageLink | null;
  previousSurah: SurahLink | null;
  nextSurah: SurahLink | null;
}

/** Neighbouring Surah, or null past either end of the mushaf. */
function surahLinkAt(num: number): SurahLink | null {
  if (num < 1 || num > SURAH_COUNT) return null;
  const entry = QURAN_DATA.surahByNum(num);
  return entry ? toSurahLink(entry) : null;
}

/**
 * Degraded-read manual jumps: adjacent GLOBAL mushaf pages around the surah
 * page being read. They address /app/page/N, the only page scheme there is.
 */
function mushafPageLink(globalPage: number): MushafPageLink | null {
  if (globalPage < 1 || globalPage > QURAN_DATA.rangeCount(RangeKind.Page)) return null;
  return { globalPage };
}

export function surahRouteNav(surah: CatalogEntry, pageGlobalPage: number): SurahRouteNav {
  return {
    previousPage: mushafPageLink(pageGlobalPage - 1),
    nextPage: mushafPageLink(pageGlobalPage + 1),
    previousSurah: surahLinkAt(surah.num - 1),
    nextSurah: surahLinkAt(surah.num + 1),
  };
}

function rangeKindFor(kind: RangeRouteKind): RangeKind {
  switch (kind) {
    case "juz":
      return RangeKind.Juz;
    case "page":
      return RangeKind.Page;
    case "hizb":
    case "rub":
      return RangeKind.HizbQuarter;
  }
}

function rangeLabel(kind: RangeRouteKind, index: number): string {
  switch (kind) {
    case "juz":
      return `Juz ${index}`;
    case "page":
      return `Page ${index}`;
    case "hizb":
      return `Hizb ${index}`;
    case "rub":
      return `Rubʿ ${index}`;
  }
}

/** Resolves the range entry a range route asks for, 404-ing on an out-of-range index. */
export function requireRangeEntry(kind: RangeRouteKind, index: number): RangeEntry {
  const entry =
    kind === "hizb"
      ? hizbRange(QURAN_DATA, index)
      : QURAN_DATA.rangeByIndex(rangeKindFor(kind), index);
  if (!entry) throw error(404, `Unknown ${kind}: ${index}`);
  return entry;
}

export function toRangePageData(
  kind: RangeRouteKind,
  index: number,
  entry: RangeEntry,
  ayahs: Ayah[],
  normalizations: SurahNormalization[],
): RangePageData {
  const surahNums = new Set(ayahs.map((ayah) => ayah.surah));
  // Division triplet of the range's first ayah, server-known for the sticky
  // indicator's first paint (plan §3.2 — the index is known at load).
  const { globalPage, juz, hizb } = positionForGlobal(QURAN_DATA, entry.startGlobal);
  return {
    kind,
    index,
    label: rangeLabel(kind, index),
    startGlobal: entry.startGlobal,
    endGlobal: entry.endGlobal,
    first: entry.first,
    last: entry.last,
    globalPage,
    juz,
    hizb,
    ayahs,
    normalizations,
    surahs: [...surahNums].flatMap((num) => surahLinkAt(num) ?? []),
  };
}
