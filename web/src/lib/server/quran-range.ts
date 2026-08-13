import type { RangePageData } from "$lib/data/quran-types";
import { requireRangeEntry, toRangePageData } from "$lib/server/quran-page-shape";

import { readRangeText } from "./quran-sqlite";

export function loadRangeData(kind: "juz" | "page", index: number): RangePageData {
  const entry = requireRangeEntry(kind, index);
  const source = readRangeText(entry.startGlobal, entry.endGlobal);
  return toRangePageData(kind, index, entry, source.ayahs, source.normalizations);
}
