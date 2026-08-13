import { readRangeText } from "./quran-sqlite";
import { requireRangeEntry, toRangePageData } from "$lib/server/quran-page-shape";
import type { RangePageData } from "$lib/data/quran-types";

export function loadRangeData(kind: "juz" | "page", index: number): RangePageData {
  const entry = requireRangeEntry(kind, index);
  const source = readRangeText(entry.startGlobal, entry.endGlobal);
  return toRangePageData(kind, index, entry, source.ayahs, source.normalizations);
}
