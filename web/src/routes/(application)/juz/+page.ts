import { QURAN_DATA_URL } from "#lib/data/quran-data-client.js";
import { createQuranData, RangeKind } from "#lib/data/quran-data.js";

import type { PageLoad } from "./$types";

export const prerender = true;

/** One of the juz's two hizbs — a real reader route (/hizb/N). */
export interface JuzHizbRow {
  index: number;
  /** First verse key of the hizb, e.g. "2:75". */
  first: string;
}

/** A verse key with its surah's name, for "Al-Fatihah 1:1"-style labels. */
export interface JuzBound {
  key: string;
  surahName: string;
}

export interface JuzSajdaRef {
  surah: number;
  ayah: number;
  kind: "obligatory" | "recommended";
}

export interface JuzIndexRow {
  index: number;
  first: JuzBound;
  last: JuzBound;
  sajdas: readonly JuzSajdaRef[];
  hizbs: readonly JuzHizbRow[];
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(QURAN_DATA_URL, {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[juz index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const ajz = quran.ranges(RangeKind.Juz);
  const hizbQuarters = quran.ranges(RangeKind.HizbQuarter);

  function bound(key: string): JuzBound {
    const surah = quran.surahByNum(Number(key.split(":")[0]));
    return { key, surahName: surah?.name ?? "" };
  }

  // 240 hizb quarters = 30 juz x 2 hizb x 4 quarters; a hizb opens on its first quarter.
  function hizbsFor(juzIndex: number): JuzHizbRow[] {
    return [1, 2].map((half) => {
      const index = (juzIndex - 1) * 2 + half;
      return { index, first: hizbQuarters[(index - 1) * 4]!.first };
    });
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
    first: bound(juz.first),
    last: bound(juz.last),
    sajdas: sajdaByJuz.get(juz.index) ?? [],
    hizbs: hizbsFor(juz.index),
  }));
  return { ajzur };
};
