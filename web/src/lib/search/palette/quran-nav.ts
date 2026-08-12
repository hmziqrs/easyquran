import type { Pathname } from "$app/types";
import {
  globalPagePathFor,
  juzPathFor,
  surahAyahPathFor,
  surahPathFor,
  type SurahRouteContext,
} from "$lib/data/quran";
import type { QuranData } from "$lib/data/quran-data";
import { reader } from "$lib/stores/reader.svelte";

export const JUZ_COUNT = 30;
export const MUSHAF_PAGE_COUNT = 604;
export const SURAH_COUNT = 114;

/**
 * The only place palette entries turn Quran coordinates into URLs. Everything
 * goes through the `*For(ctx, ...)` helpers, so a jump made while reading a
 * translation stays in that translation.
 */
export const juzHref = (ctx: SurahRouteContext, n: number): Pathname => juzPathFor(ctx, n);

export const pageHref = (ctx: SurahRouteContext, n: number): Pathname => globalPagePathFor(ctx, n);

export function surahHref(
  ctx: SurahRouteContext,
  quranData: QuranData,
  surah: number,
): Pathname | null {
  const entry = quranData.surahByNum(surah);
  return entry ? surahPathFor(ctx, entry) : null;
}

/** Null when the coordinate has no reachable page — never link into nowhere. */
export function ayahHref(
  ctx: SurahRouteContext,
  quranData: QuranData,
  surah: number,
  ayah: number,
): Pathname | null {
  const entry = quranData.surahByNum(surah);
  if (!entry) return null;
  const localPage = quranData.surahLocalPageForAyah(surah, ayah);
  return localPage ? surahAyahPathFor(ctx, entry, localPage.localPage, ayah) : null;
}

/**
 * Side effect to pair with an ayah href: the reader tracks the opened verse for
 * "continue reading" and for revealing the row it lands on.
 */
export const openVerse =
  (surah: number, ayah: number): (() => void) =>
  () =>
    reader.openVerse(surah, ayah);

export const surahDetail = (surah: {
  transliteration: string;
  meaning: string;
  ayahCount: number;
}): string => `${surah.transliteration} · ${surah.meaning} · ${surah.ayahCount} verses`;
