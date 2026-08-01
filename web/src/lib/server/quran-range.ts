import { error } from "@sveltejs/kit";
import { NAVIGATION } from "$lib/data/quran-meta";
import { readRangeText } from "./quran-sqlite";
import type { RangePageData } from "$lib/data/quran-types";

export function loadRangeData(kind: "juz" | "page", index: number): RangePageData {
  const list = kind === "juz" ? NAVIGATION.juz : NAVIGATION.page;
  const entry = list[index - 1];
  if (!entry) throw error(404, `Unknown ${kind}: ${index}`);
  const source = readRangeText(entry.startGlobal, entry.endGlobal);
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
  };
}
