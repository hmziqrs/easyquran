import { asset } from "$app/paths";
import { createQuranData, RangeKind } from "$lib/data/quran-data";

import type { PageLoad } from "./$types";

export const prerender = true;

export interface JuzQuarterRow {
  /** First verse key of the quarter, e.g. "2:22". */
  first: string;
  /** Last verse key of the quarter, e.g. "3:11". */
  last: string;
  /** Mushaf page containing the quarter's opening verse. */
  page: number;
}

export interface JuzSajdaRef {
  surah: number;
  ayah: number;
  kind: "obligatory" | "recommended";
}

export interface JuzIndexRow {
  index: number;
  first: string;
  last: string;
  sajdas: readonly JuzSajdaRef[];
  quarters: readonly JuzQuarterRow[];
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(asset("/quran-meta/quran-data.json"), {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[juz index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const ajz = quran.ranges(RangeKind.Juz);
  const hizbQuarters = quran.ranges(RangeKind.HizbQuarter);
  const pages = quran.ranges(RangeKind.Page);

  function pageContaining(globalIndex: number): number {
    const found = pages.findIndex(
      (page) => globalIndex >= page.startGlobal && globalIndex <= page.endGlobal,
    );
    return found + 1;
  }

  // 240 hizb quarters = 30 juz x 8; a rub' al-juz (quarter) pairs two of them.
  function quarterFor(juzIndex: number, quarter: number): JuzQuarterRow {
    const base = (juzIndex - 1) * 8 + (quarter - 1) * 2;
    const first = hizbQuarters[base]!;
    const second = hizbQuarters[base + 1]!;
    return { first: first.first, last: second.last, page: pageContaining(first.startGlobal) };
  }

  const sajdaByJuz = new Map<number, JuzSajdaRef[]>();
  for (const juz of ajz) {
    for (const sajda of quran.sajdas()) {
      if (sajda.globalIndex >= juz.startGlobal && sajda.globalIndex <= juz.endGlobal) {
        const refs = sajdaByJuz.get(juz.index) ?? [];
        refs.push({ surah: sajda.surah, ayah: sajda.ayah, kind: sajda.kind });
        sajdaByJuz.set(juz.index, refs);
      }
    }
  }

  const ajzur: JuzIndexRow[] = ajz.map((juz) => ({
    index: juz.index,
    first: juz.first,
    last: juz.last,
    sajdas: sajdaByJuz.get(juz.index) ?? [],
    quarters: [1, 2, 3, 4].map((quarter) => quarterFor(juz.index, quarter)),
  }));
  return { ajzur };
};
