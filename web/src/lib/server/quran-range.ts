import type { RangePageData, RangeRouteKind } from "#lib/data/quran-types.js";
import { requireRangeEntry, toRangePageData } from "#lib/server/quran-page-shape.js";

import { readRangeText } from "./quran-sqlite";

export function loadRangeData(kind: RangeRouteKind, index: number): RangePageData {
  const entry = requireRangeEntry(kind, index);
  const source = readRangeText(entry.startGlobal, entry.endGlobal);
  return toRangePageData(kind, index, entry, source.ayahs, source.normalizations);
}
