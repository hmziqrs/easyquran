import { error } from "@sveltejs/kit";
import { RangeKind } from "$lib/data/quran-data";
import { QURAN_DATA, toSurahLink } from "$lib/server/quran-data";
import { readRangeText } from "./quran-sqlite";
import type { RangePageData } from "$lib/data/quran-types";

export function loadRangeData(kind: "juz" | "page", index: number): RangePageData {
  const rangeKind = kind === "juz" ? RangeKind.Juz : RangeKind.Page;
  const entry = QURAN_DATA.rangeByIndex(rangeKind, index);
  if (!entry) throw error(404, `Unknown ${kind}: ${index}`);
  const source = readRangeText(entry.startGlobal, entry.endGlobal);
  const surahNums = new Set(source.ayahs.map((ayah) => ayah.surah));
  return {
    kind,
    index,
    label: `${kind === "juz" ? "Juz" : "Page"} ${index}`,
    startGlobal: entry.startGlobal,
    endGlobal: entry.endGlobal,
    first: entry.first,
    last: entry.last,
    ayahs: source.ayahs,
    normalizations: source.normalizations,
    surahs: [...surahNums].map((num) => toSurahLink(QURAN_DATA.surahByNum(num)!)),
  };
}
