// quran-range.ts — SERVER-ONLY loader for a prerendered juz/page range.
// Resolves the range bounds from the build-time NAVIGATION metadata and reads
// the verbatim Uthmani ayahs from quran-uthmani.sqlite via node:sqlite.
import { error } from "@sveltejs/kit";
import { NAVIGATION } from "$lib/data/quran-meta";
import { readRangeAyahs } from "./quran-sqlite";
import type { RangePageData } from "$lib/data/quran-types";

/** Build the prerender data for `/app/juz/[n]` or `/app/page/[n]`. */
export function loadRangeData(kind: "juz" | "page", index: number): RangePageData {
  const list = kind === "juz" ? NAVIGATION.juz : NAVIGATION.page;
  const entry = list[index - 1];
  if (!entry) throw error(404, `Unknown ${kind}: ${index}`);
  return {
    kind,
    index,
    label: `${kind === "juz" ? "Juz" : "Page"} ${index}`,
    startGlobal: entry.startGlobal,
    endGlobal: entry.endGlobal,
    first: entry.first,
    last: entry.last,
    ayahs: readRangeAyahs(entry.startGlobal, entry.endGlobal),
  };
}
