import { translationIdFromSegments } from "$lib/data/quran";
import { RangeKind } from "$lib/data/quran-data";
import { hizbRange } from "$lib/data/mushaf-divisions";
import { TRANSLATION_BY_ID, type BakedTranslationMetadata } from "$lib/data/translations";
import { isUiLocale, type UiDirection, type UiLocale } from "$lib/i18n/locales";
import { QURAN_DATA } from "$lib/server/quran-data";

interface ReaderIndexRoute {
  readonly type: "index";
  readonly page: "home" | "juz" | "surah" | "pages";
}

/** Range route segments; hizb/rub address the baked hizb-quarter series. */
export type ReaderRangeSegment = "page" | "juz" | "hizb" | "rub";

interface ArabicReaderRoute {
  readonly type: "arabic";
  readonly cacheKind: "surah" | ReaderRangeSegment;
  readonly index: number;
}

interface TranslationReaderRoute {
  readonly type: "translation";
  readonly sourceId: string;
  readonly contentLanguage: string;
  readonly contentDirection: UiDirection;
  readonly cacheKind: "surah" | ReaderRangeSegment;
  readonly index: number;
}

export type ParsedReaderRoute = ReaderIndexRoute | ArabicReaderRoute | TranslationReaderRoute;

const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u;
const CONTENT_LANGUAGE_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u;
// Keep in lockstep with the TRANSLATOR_SEGMENT copies in src/lib/i18n/reader.ts, src/hooks.ts,
// and src/lib/accept-parse.ts (baked ids include underscore translator segments,
// e.g. quranenc.en.hilali_khan).
const TRANSLATOR_SEGMENT = /^[a-z0-9]+(?:[._-][a-z0-9]+)*$/u;
const POSITIVE_INTEGER = /^[1-9][0-9]*$/u;

export function localizedReaderLocale(pathname: string): UiLocale | null {
  const match = /^\/(en|ar)\/app(?:\/|$)/u.exec(pathname);
  return match && isUiLocale(match[1]) ? match[1] : null;
}

function positiveInteger(value: string): number | null {
  if (!POSITIVE_INTEGER.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function translation(
  lang: string,
  translator: string,
): { sourceId: string; metadata: BakedTranslationMetadata } | null {
  if (!CONTENT_LANGUAGE_SEGMENT.test(lang) || !TRANSLATOR_SEGMENT.test(translator)) return null;
  const sourceId = translationIdFromSegments(lang, translator);
  const metadata = TRANSLATION_BY_ID.get(sourceId);
  return metadata ? { sourceId, metadata } : null;
}

function surahRoute(slug: string): ArabicReaderRoute | null {
  const surah = QURAN_DATA.surahBySlug(slug);
  if (!surah) return null;
  return { type: "arabic", cacheKind: "surah", index: surah.num };
}

function translationSurahRoute(
  slug: string,
  lang: string,
  translator: string,
): TranslationReaderRoute | null {
  const surah = QURAN_DATA.surahBySlug(slug);
  const source = translation(lang, translator);
  if (!surah || !source) return null;
  return {
    type: "translation",
    sourceId: source.sourceId,
    contentLanguage: source.metadata.languageCode,
    contentDirection: source.metadata.direction,
    cacheKind: "surah",
    index: surah.num,
  };
}

function rangeRoute(segment: ReaderRangeSegment, value: string): ArabicReaderRoute | null {
  const index = positiveInteger(value);
  if (index === null || !rangeIndexInRange(segment, index)) return null;
  return { type: "arabic", cacheKind: segment, index };
}

function rangeKindFromSegment(segment: string): ReaderRangeSegment | null {
  if (segment === "page" || segment === "juz" || segment === "hizb" || segment === "rub") {
    return segment;
  }
  return null;
}

function rangeIndexInRange(segment: ReaderRangeSegment, index: number): boolean {
  switch (segment) {
    case "page":
      return QURAN_DATA.rangeByIndex(RangeKind.Page, index) !== undefined;
    case "juz":
      return QURAN_DATA.rangeByIndex(RangeKind.Juz, index) !== undefined;
    case "rub":
      return QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, index) !== undefined;
    case "hizb":
      return hizbRange(QURAN_DATA, index) !== undefined;
  }
}

function translationRangeRoute(
  lang: string,
  translator: string,
  segment: ReaderRangeSegment,
  value: string,
): TranslationReaderRoute | null {
  const source = translation(lang, translator);
  const index = positiveInteger(value);
  if (!source || index === null || !rangeIndexInRange(segment, index)) return null;
  return {
    type: "translation",
    sourceId: source.sourceId,
    contentLanguage: source.metadata.languageCode,
    contentDirection: source.metadata.direction,
    cacheKind: segment,
    index,
  };
}

/** Parse one canonical, de-localized reader pathname. No query or fragment accepted. */
export function parseReaderPath(pathname: string): ParsedReaderRoute | null {
  if (pathname.length > 256 || pathname.includes("%") || pathname.includes("//")) return null;
  if (pathname === "/app") return { type: "index", page: "home" };
  if (pathname === "/app/juz") return { type: "index", page: "juz" };
  if (pathname === "/app/surah") return { type: "index", page: "surah" };
  if (pathname === "/app/pages") return { type: "index", page: "pages" };

  let match = /^\/app\/([^/]+)\/t\/([^/]+)\/([^/]+)$/u.exec(pathname);
  if (match) return translationSurahRoute(match[1]!, match[2]!, match[3]!);

  match = /^\/app\/t\/([^/]+)\/([^/]+)\/(page|juz|hizb|rub)\/([^/]+)$/u.exec(pathname);
  if (match) {
    const kind = rangeKindFromSegment(match[3]!);
    if (!kind) return null;
    return translationRangeRoute(match[1]!, match[2]!, kind, match[4]!);
  }

  match = /^\/app\/(page|juz|hizb|rub)\/([^/]+)$/u.exec(pathname);
  if (match) {
    const kind = rangeKindFromSegment(match[1]!);
    return kind ? rangeRoute(kind, match[2]!) : null;
  }

  match = /^\/app\/([^/]+)$/u.exec(pathname);
  return match ? surahRoute(match[1]!) : null;
}

export interface SurahLocalRedirectTarget {
  /** Bare canonical path of the surah root (Arabic or translated). */
  path: string;
  /** `#ayah-{surah}-{startAyah}` preserving the spread's first ayah; empty for local page 1. */
  fragment: string;
}

const ARABIC_SURAH_LOCAL_PAGE = /^\/app\/([^/]+)\/page\/([1-9][0-9]*)$/u;
const TRANSLATED_SURAH_LOCAL_PAGE = /^\/app\/([^/]+)\/t\/([^/]+)\/([^/]+)\/page\/([1-9][0-9]*)$/u;

/**
 * Permanent redirect target for a removed surah-local page URL (D1). Pure —
 * no event, no query/fragment handling — so the hooks branch owns localization
 * and suffix preservation. Unknown slug, unknown translation, or out-of-range
 * local page yields null (caller falls through to 404).
 */
export function surahLocalRedirectTarget(pathname: string): SurahLocalRedirectTarget | null {
  const translated = TRANSLATED_SURAH_LOCAL_PAGE.exec(pathname);
  const arabic = translated ? null : ARABIC_SURAH_LOCAL_PAGE.exec(pathname);
  if (!translated && !arabic) return null;
  const match = translated ?? arabic!;
  const slug = match[1]!;
  const surah = QURAN_DATA.surahBySlug(slug);
  if (!surah) return null;

  let path: string;
  let localPageValue: string;
  if (translated) {
    if (!translation(match[2]!, match[3]!)) return null;
    path = `/app/${surah.slug}/t/${match[2]}/${match[3]}`;
    localPageValue = match[4]!;
  } else {
    path = `/app/${surah.slug}`;
    localPageValue = match[2]!;
  }

  const localPage = Number(localPageValue);
  if (localPage === 1) return { path, fragment: "" };
  const page = QURAN_DATA.surahLocalPage(surah.num, localPage);
  if (!page) return null;
  return { path, fragment: `#ayah-${surah.num}-${page.startAyah}` };
}

function requiredParam(
  params: Record<string, string | undefined>,
  name: string,
  pattern: RegExp,
): string | null {
  const value = params[name];
  return value && pattern.test(value) ? value : null;
}

/** Parse the resolved internal SvelteKit route. Use before translated HTML cache access. */
export function parseReaderRoute(
  routeId: string | null,
  params: Record<string, string | undefined>,
): ParsedReaderRoute | null {
  if (!routeId) return null;
  const marker = "/app";
  const markerIndex = routeId.indexOf(marker);
  if (markerIndex < 0) return null;
  const routePattern = routeId.slice(markerIndex).replace(/\.md$/u, "");
  const surah = requiredParam(params, "surah", SURAH_SEGMENT);
  const lang = requiredParam(params, "lang", CONTENT_LANGUAGE_SEGMENT);
  const translator = requiredParam(params, "translator", TRANSLATOR_SEGMENT);
  const n = requiredParam(params, "n", POSITIVE_INTEGER);

  switch (routePattern) {
    case "/app":
      return parseReaderPath("/app");
    case "/app/juz":
      return parseReaderPath("/app/juz");
    case "/app/surah":
      return parseReaderPath("/app/surah");
    case "/app/pages":
      return parseReaderPath("/app/pages");
    case "/app/[surah]":
      return surah ? parseReaderPath(`/app/${surah}`) : null;
    case "/app/page/[n]":
      return n ? parseReaderPath(`/app/page/${n}`) : null;
    case "/app/juz/[n]":
      return n ? parseReaderPath(`/app/juz/${n}`) : null;
    case "/app/hizb/[n]":
      return n ? parseReaderPath(`/app/hizb/${n}`) : null;
    case "/app/rub/[n]":
      return n ? parseReaderPath(`/app/rub/${n}`) : null;
    case "/app/[surah]/t/[lang]/[translator]":
      return surah && lang && translator
        ? parseReaderPath(`/app/${surah}/t/${lang}/${translator}`)
        : null;
    case "/app/t/[lang]/[translator]/page/[n]":
      return lang && translator && n
        ? parseReaderPath(`/app/t/${lang}/${translator}/page/${n}`)
        : null;
    case "/app/t/[lang]/[translator]/juz/[n]":
      return lang && translator && n
        ? parseReaderPath(`/app/t/${lang}/${translator}/juz/${n}`)
        : null;
    case "/app/t/[lang]/[translator]/hizb/[n]":
      return lang && translator && n
        ? parseReaderPath(`/app/t/${lang}/${translator}/hizb/${n}`)
        : null;
    case "/app/t/[lang]/[translator]/rub/[n]":
      return lang && translator && n
        ? parseReaderPath(`/app/t/${lang}/${translator}/rub/${n}`)
        : null;
    default:
      return null;
  }
}
