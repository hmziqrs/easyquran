import type { SurahRouteContext } from "$lib/data/quran";
import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  rubPathFor,
  surahPathFor,
} from "$lib/data/quran";
import { RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";
import { HIZB_COUNT, RUB_COUNT } from "$lib/data/mushaf-divisions";
import type { PublicHref } from "$lib/i18n/public-href";
import type { QuranReaderHref } from "$lib/i18n/reader";

const ARABIC: SurahRouteContext = { kind: "arabic" };

export const READER_GLOBAL_PAGE_COUNT = RANGE_COUNTS[RangeKind.Page];
export const READER_JUZ_COUNT = RANGE_COUNTS[RangeKind.Juz];
export const READER_HIZB_COUNT = HIZB_COUNT;
export const READER_RUB_COUNT = RUB_COUNT;

interface ReaderSurah {
  num: number;
  slug: string;
}

export interface ReaderPrerenderSource {
  readonly surahs: readonly ReaderSurah[];
}

export type ReaderPrerenderEntry =
  | { kind: "surah"; surah: ReaderSurah }
  | { kind: "global-page"; globalPage: number }
  | { kind: "juz"; juz: number }
  | { kind: "hizb"; hizb: number }
  | { kind: "rub"; rub: number };

export type ReaderHrefFor<Locale extends string> = (
  locale: Locale,
  quranHref: QuranReaderHref,
) => PublicHref;

export type ReaderEntryHrefFor<Locale extends string> = (
  locale: Locale,
  page: "juz-index" | "surah-index" | "pages-index",
) => PublicHref;

/** Existing Arabic-source entries(), represented once for sitemap and SSG discovery. */
export function readerPrerenderEntries(source: ReaderPrerenderSource): ReaderPrerenderEntry[] {
  const entries: ReaderPrerenderEntry[] = [];

  for (const surah of source.surahs) {
    entries.push({ kind: "surah", surah });
  }
  for (let globalPage = 1; globalPage <= READER_GLOBAL_PAGE_COUNT; globalPage += 1) {
    entries.push({ kind: "global-page", globalPage });
  }
  for (let juz = 1; juz <= READER_JUZ_COUNT; juz += 1) {
    entries.push({ kind: "juz", juz });
  }
  for (let hizb = 1; hizb <= READER_HIZB_COUNT; hizb += 1) {
    entries.push({ kind: "hizb", hizb });
  }
  for (let rub = 1; rub <= READER_RUB_COUNT; rub += 1) {
    entries.push({ kind: "rub", rub });
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
    case "global-page":
      return globalPagePathFor(context, entry.globalPage);
    case "juz":
      return juzPathFor(context, entry.juz);
    case "hizb":
      return hizbPathFor(context, entry.hizb);
    case "rub":
      return rubPathFor(context, entry.rub);
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
    readerEntryHrefFor(locale, "juz-index"),
    readerEntryHrefFor(locale, "surah-index"),
    readerEntryHrefFor(locale, "pages-index"),
  ]);

  if (new Set(hrefs).size !== hrefs.length) {
    throw new Error("[reader-prerender] duplicate localized reader href");
  }
  return hrefs;
}
