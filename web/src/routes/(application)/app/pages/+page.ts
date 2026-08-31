import { asset } from "$app/paths";
import { createQuranData, RangeKind } from "$lib/data/quran-data";

import type { PageLoad } from "./$types";

export const prerender = true;

export interface PageSurahRef {
  num: number;
  name: string;
}

export interface PageSajdaRef {
  surah: number;
  ayah: number;
}

export interface PageIndexRow {
  index: number;
  /** First verse key on the mushaf page, e.g. "2:142". */
  first: string;
  /** Last verse key on the mushaf page, e.g. "3:10". */
  last: string;
  /** Surahs the page draws from (pages can span two or more). */
  surahs: readonly PageSurahRef[];
  /** Surahs that *begin* on this page — the mushaf's surah-openers. */
  surahStarts: readonly number[];
  /** Sajda verses falling on this page. */
  sajdas: readonly PageSajdaRef[];
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(asset("/quran-meta/quran-data.json"), {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[pages index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const pages = quran.ranges(RangeKind.Page);
  const surahs = quran.surahs;
  const sajdas = quran.sajdas();

  const rows: PageIndexRow[] = [];
  let sajdaCursor = 0;
  for (const page of pages) {
    // Surahs are ordered by startGlobal; the first surah ending before this page
    // bounds the scan for every later page too.
    let first = 0;
    while (
      first < surahs.length &&
      surahs[first]!.startGlobal + surahs[first]!.ayahCount - 1 < page.startGlobal
    ) {
      first += 1;
    }
    const covered: PageSurahRef[] = [];
    const starts: number[] = [];
    for (let i = first; i < surahs.length; i += 1) {
      const surah = surahs[i]!;
      if (surah.startGlobal > page.endGlobal) break;
      covered.push({ num: surah.num, name: surah.name });
      if (surah.startGlobal >= page.startGlobal) starts.push(surah.num);
    }

    while (sajdaCursor < sajdas.length && sajdas[sajdaCursor]!.globalIndex < page.startGlobal) {
      sajdaCursor += 1;
    }
    const pageSajdas: PageSajdaRef[] = [];
    for (let j = sajdaCursor; j < sajdas.length; j += 1) {
      const sajda = sajdas[j]!;
      if (sajda.globalIndex > page.endGlobal) break;
      pageSajdas.push({ surah: sajda.surah, ayah: sajda.ayah });
    }

    rows.push({
      index: page.index,
      first: page.first,
      last: page.last,
      surahs: covered,
      surahStarts: starts,
      sajdas: pageSajdas,
    });
  }
  return { rows };
};
