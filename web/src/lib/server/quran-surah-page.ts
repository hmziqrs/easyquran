import type { CatalogEntry, SurahLocalPageData, SurahRouteData } from "$lib/data/quran-types";
import { QURAN_DATA, toSurahRenderMetadata } from "$lib/server/quran-data";
import { surahRouteNav } from "$lib/server/quran-page-shape";
import { readRangeText } from "$lib/server/quran-sqlite";

const ARABIC_CTX = { kind: "arabic" } as const;

export function readSurahLocalPageData(
  surah: CatalogEntry,
  localPage: number,
): SurahLocalPageData | undefined {
  const page = QURAN_DATA.surahLocalPage(surah.num, localPage);
  if (!page) return undefined;
  const range = readRangeText(page.startGlobal, page.endGlobal);
  const normalization = range.normalizations.find((value) => value.surah === surah.num);
  if (
    !normalization ||
    range.ayahs.length !== page.endAyah - page.startAyah + 1 ||
    range.ayahs.some((ayah) => ayah.surah !== surah.num)
  ) {
    throw new Error(`[quran-sqlite] invalid Surah-local page ${surah.num}:${localPage}`);
  }
  return {
    surah: toSurahRenderMetadata(surah),
    page,
    pageCount: QURAN_DATA.surahLocalPageCount(surah.num),
    ayahs: range.ayahs,
    normalization,
  };
}

export function readSurahRouteData(
  surah: CatalogEntry,
  localPage: number,
): SurahRouteData | undefined {
  const pageData = readSurahLocalPageData(surah, localPage);
  if (!pageData) return undefined;
  return { pageData, ...surahRouteNav(ARABIC_CTX, surah, localPage, pageData.pageCount) };
}
