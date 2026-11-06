import type { CatalogEntry } from "#lib/data/quran-types.js";

/** Sidebar surah order: the mushaf (default) or the order the surahs were revealed. */
export const SurahSort = {
  Mushaf: "mushaf",
  Revelation: "revelation",
} as const;
export type SurahSort = (typeof SurahSort)[keyof typeof SurahSort];

/**
 * Surahs by `revelationOrder` (1 = first revealed). Ties keep mushaf order, so
 * the result is a total order and the same on every client. Returns a copy.
 */
export function surahsInRevelationOrder(surahs: readonly CatalogEntry[]): CatalogEntry[] {
  return surahs.toSorted((a, b) => a.revelationOrder - b.revelationOrder || a.num - b.num);
}

/** The list the sidebar draws for a sort; mushaf order is the input array itself. */
export function surahsInSortOrder(
  surahs: readonly CatalogEntry[],
  sort: SurahSort,
): readonly CatalogEntry[] {
  if (sort === SurahSort.Revelation) return surahsInRevelationOrder(surahs);
  return surahs;
}
