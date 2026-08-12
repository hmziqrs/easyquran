import {
  globalPagePathFor,
  juzPathFor,
  surahLocalPagePathFor,
  surahPathFor,
  type SurahRouteContext,
} from "$lib/data/quran";
import type { QuranReaderHref } from "$lib/i18n/reader";
import type { PublicHref } from "$lib/i18n/public-href";

const ARABIC: SurahRouteContext = { kind: "arabic" };

export const READER_GLOBAL_PAGE_COUNT = 604;
export const READER_JUZ_COUNT = 30;

interface ReaderSurah {
  num: number;
  slug: string;
}

export interface ReaderPrerenderSource {
  readonly surahs: readonly ReaderSurah[];
  surahLocalPageCount(surah: number): number;
}

export type ReaderPrerenderEntry =
  | { kind: "surah"; surah: ReaderSurah }
  | { kind: "surah-local-page"; surah: ReaderSurah; localPage: number }
  | { kind: "global-page"; globalPage: number }
  | { kind: "juz"; juz: number };

export type ReaderHrefFor<Locale extends string> = (
  locale: Locale,
  quranHref: QuranReaderHref,
) => PublicHref;

export type ReaderEntryHrefFor<Locale extends string> = (
  locale: Locale,
  page: "home" | "juz-index",
) => PublicHref;

/** Existing Arabic-source entries(), represented once for sitemap and SSG discovery. */
export function readerPrerenderEntries(source: ReaderPrerenderSource): ReaderPrerenderEntry[] {
  const entries: ReaderPrerenderEntry[] = [];

  for (const surah of source.surahs) {
    entries.push({ kind: "surah", surah });
  }
  for (const surah of source.surahs) {
    const pageCount = source.surahLocalPageCount(surah.num);
    for (let localPage = 2; localPage <= pageCount; localPage += 1) {
      entries.push({ kind: "surah-local-page", surah, localPage });
    }
  }
  for (let globalPage = 1; globalPage <= READER_GLOBAL_PAGE_COUNT; globalPage += 1) {
    entries.push({ kind: "global-page", globalPage });
  }
  for (let juz = 1; juz <= READER_JUZ_COUNT; juz += 1) {
    entries.push({ kind: "juz", juz });
  }

  return entries;
}

/** Uses route-aware helpers for Arabic and translation source contexts. */
export function quranHrefForPrerenderEntry(
  entry: ReaderPrerenderEntry,
  context: SurahRouteContext,
): QuranReaderHref {
  switch (entry.kind) {
    case "surah":
      return surahPathFor(context, entry.surah);
    case "surah-local-page":
      return surahLocalPagePathFor(context, entry.surah, entry.localPage);
    case "global-page":
      return globalPagePathFor(context, entry.globalPage);
    case "juz":
      return juzPathFor(context, entry.juz);
  }
}

/**
 * Build-only discovery set: Arabic-source entries x bounded UI locales.
 * Translation contexts never enter this function.
 */
export function readerPrerenderHrefs<Locale extends string>(
  source: ReaderPrerenderSource,
  locales: readonly Locale[],
  readerHrefFor: ReaderHrefFor<Locale>,
  readerEntryHrefFor: ReaderEntryHrefFor<Locale>,
): PublicHref[] {
  const entries = readerPrerenderEntries(source);
  const hrefs = locales.flatMap((locale) => [
    ...entries.map((entry) => readerHrefFor(locale, quranHrefForPrerenderEntry(entry, ARABIC))),
    readerEntryHrefFor(locale, "home"),
    readerEntryHrefFor(locale, "juz-index"),
  ]);

  if (new Set(hrefs).size !== hrefs.length) {
    throw new Error("[reader-prerender] duplicate localized reader href");
  }
  return hrefs;
}
