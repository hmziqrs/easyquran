import { surahLocalPagePath } from "$lib/data/quran";
import type { CatalogEntry, SurahLocalPageData, SurahRouteData } from "$lib/data/quran-types";
import { QURAN_DATA, toSurahLink, toSurahRenderMetadata } from "$lib/server/quran-data";
import { readRangeText, validateReaderSource } from "$lib/server/quran-sqlite";

const source = validateReaderSource();
if (!source.ok) {
  throw new Error(
    `[quran-sqlite] Uthmani source validation failed: rows=${source.rows} (want 6236), ` +
      `sha256=${source.sha256}. The DB and its registered digest are out of sync.`,
  );
}

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
  const pageCount = pageData.pageCount;
  return {
    pageData,
    previousPage:
      localPage > 1
        ? { localPage: localPage - 1, href: surahLocalPagePath(surah, localPage - 1) }
        : null,
    nextPage:
      localPage < pageCount
        ? { localPage: localPage + 1, href: surahLocalPagePath(surah, localPage + 1) }
        : null,
    previousSurah: surah.num > 1 ? toSurahLink(QURAN_DATA.surahByNum(surah.num - 1)!) : null,
    nextSurah: surah.num < 114 ? toSurahLink(QURAN_DATA.surahByNum(surah.num + 1)!) : null,
  };
}
