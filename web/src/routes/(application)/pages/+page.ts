import { QURAN_DATA_URL } from "#lib/data/quran-data-client.js";
import { createQuranData, RangeKind } from "#lib/data/quran-data.js";

import type { PageLoad } from "./$types";

export const prerender = true;

export interface PageSurahRef {
  num: number;
  name: string;
  /** Arabic surah name, shown on the card like the surah index does. */
  arabic: string;
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
  /** Juz holding the page's first verse — the index groups pages by juz. */
  juz: number;
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(QURAN_DATA_URL, {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[pages index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const pages = quran.ranges(RangeKind.Page);
  const ajz = quran.ranges(RangeKind.Juz);
  const surahs = quran.surahs;
  const sajdas = quran.sajdas();

  const rows: PageIndexRow[] = [];
  let sajdaCursor = 0;
  let juzCursor = 0;
  for (const page of pages) {
    while (juzCursor < ajz.length - 1 && ajz[juzCursor]!.endGlobal < page.startGlobal) {
      juzCursor += 1;
    }
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
      covered.push({ num: surah.num, name: surah.name, arabic: surah.arabic });
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
      juz: ajz[juzCursor]?.index ?? 1,
    });
  }
  return { rows };
};
