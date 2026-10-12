import { hizbRange } from "#lib/data/mushaf-divisions.js";
import { RangeKind } from "#lib/data/quran-data.js";
import { translationIdFromSegments } from "#lib/data/quran.js";
import { snippetForSlug, SNIPPET_SLUG_SEGMENT } from "#lib/data/snippets.js";
import { TRANSLATION_BY_ID, type BakedTranslationMetadata } from "#lib/data/translations.js";
import { type UiDirection, type UiLocale } from "#lib/i18n/locales.js";
import { isTafsirTranslation } from "#lib/quran/text-marks.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

interface ReaderIndexRoute {
  readonly type: "index";
  readonly page: "juz" | "surah" | "pages";
}

/** Range route segments; hizb addresses the baked hizb-quarter series (four per hizb). */
export type ReaderRangeSegment = "page" | "juz" | "hizb";

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

/** Curated snippet detail (`/azkar/{slug}`, Feature 7): Arabic, prerenderable. */
interface SnippetReaderRoute {
  readonly type: "snippet";
  readonly slug: string;
}

/**
 * Translated snippet detail (`/t/{lang}/{translator}/azkar/{slug}`, §11.6):
 * SSR + 7-day disk cache, never SSG — the cache key rides `sourceId` +
 * the `azkar` kind + the slug.
 */
interface TranslatedSnippetReaderRoute {
  readonly type: "translated-snippet";
  readonly sourceId: string;
  readonly contentLanguage: string;
  readonly contentDirection: UiDirection;
  readonly slug: string;
}

export type ParsedReaderRoute =
  | ReaderIndexRoute
  | ArabicReaderRoute
  | TranslationReaderRoute
  | SnippetReaderRoute
  | TranslatedSnippetReaderRoute;

const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u;
const CONTENT_LANGUAGE_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u;
// Keep in lockstep with the TRANSLATOR_SEGMENT copies in src/lib/i18n/reader.ts, src/hooks.ts,
// and src/lib/accept-parse.ts (baked ids include underscore translator segments,
// e.g. quranenc.en.hilali_khan).
const TRANSLATOR_SEGMENT = /^[a-z0-9]+(?:[._-][a-z0-9]+)*$/u;
const POSITIVE_INTEGER = /^[1-9][0-9]*$/u;

/** Scheme A: only `ar` is prefix-detectable (`/ar/**` application paths);
 * `en` is the absence of a prefix. The bare `/ar` marketing home is NOT an
 * application path and stays null (the marketing gate owns it). */
export function localizedReaderLocale(pathname: string): UiLocale | null {
  return /^\/ar\/.+/u.test(pathname) ? "ar" : null;
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
  if (!metadata) return null;
  // Tafsir works are commentary, never reading translations (TRX-01): the
  // parser refuses them so no reader route can render them in the verse
  // position — hooks.server 308s them onto the Arabic reader with the tafsir
  // stacked instead.
  if (isTafsirTranslation(sourceId)) return null;
  return { sourceId, metadata };
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
  if (segment === "page" || segment === "juz" || segment === "hizb") {
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

function snippetRoute(slug: string): SnippetReaderRoute | null {
  if (!SNIPPET_SLUG_SEGMENT.test(slug) || !snippetForSlug(slug)) return null;
  return { type: "snippet", slug };
}

function translatedSnippetRoute(
  lang: string,
  translator: string,
  slug: string,
): TranslatedSnippetReaderRoute | null {
  const source = translation(lang, translator);
  if (!source) return null;
  const arabic = snippetRoute(slug);
  if (!arabic) return null;
  return {
    type: "translated-snippet",
    sourceId: source.sourceId,
    contentLanguage: source.metadata.languageCode,
    contentDirection: source.metadata.direction,
    slug: arabic.slug,
  };
}

/** Parse one canonical, prefix-less reader pathname. No query or fragment accepted. */
export function parseReaderPath(pathname: string): ParsedReaderRoute | null {
  if (pathname.length > 256 || pathname.includes("%") || pathname.includes("//")) return null;
  if (pathname === "/juz") return { type: "index", page: "juz" };
  if (pathname === "/surah") return { type: "index", page: "surah" };
  if (pathname === "/pages") return { type: "index", page: "pages" };

  let match = /^\/([^/]+)\/t\/([^/]+)\/([^/]+)$/u.exec(pathname);
  if (match) return translationSurahRoute(match[1]!, match[2]!, match[3]!);

  match = /^\/t\/([^/]+)\/([^/]+)\/azkar\/([^/]+)$/u.exec(pathname);
  if (match) return translatedSnippetRoute(match[1]!, match[2]!, match[3]!);

  match = /^\/azkar\/([^/]+)$/u.exec(pathname);
  if (match) return snippetRoute(match[1]!);

  match = /^\/t\/([^/]+)\/([^/]+)\/(page|juz|hizb)\/([^/]+)$/u.exec(pathname);
  if (match) {
    const kind = rangeKindFromSegment(match[3]!);
    if (!kind) return null;
    return translationRangeRoute(match[1]!, match[2]!, kind, match[4]!);
  }

  match = /^\/(page|juz|hizb)\/([^/]+)$/u.exec(pathname);
  if (match) {
    const kind = rangeKindFromSegment(match[1]!);
    return kind ? rangeRoute(kind, match[2]!) : null;
  }

  match = /^\/([^/]+)$/u.exec(pathname);
  return match ? surahRoute(match[1]!) : null;
}

export interface SurahLocalRedirectTarget {
  /** Bare canonical path of the surah root (Arabic or translated). */
  path: string;
  /** `#ayah-{surah}-{startAyah}` preserving the spread's first ayah; empty for local page 1. */
  fragment: string;
}

const ARABIC_SURAH_LOCAL_PAGE = /^\/([^/]+)\/page\/([1-9][0-9]*)$/u;
const TRANSLATED_SURAH_LOCAL_PAGE = /^\/([^/]+)\/t\/([^/]+)\/([^/]+)\/page\/([1-9][0-9]*)$/u;

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
    path = `/${surah.slug}/t/${match[2]}/${match[3]}`;
    localPageValue = match[4]!;
  } else {
    path = `/${surah.slug}`;
    localPageValue = match[2]!;
  }

  const localPage = Number(localPageValue);
  if (localPage === 1) return { path, fragment: "" };
  const page = QURAN_DATA.surahLocalPage(surah.num, localPage);
  if (!page) return null;
  return { path, fragment: `#ayah-${surah.num}-${page.startAyah}` };
}

const ARABIC_RUB = /^\/rub\/([1-9][0-9]*)(\.md)?$/u;
const TRANSLATED_RUB = /^\/t\/([^/]+)\/([^/]+)\/rub\/([1-9][0-9]*)(\.md)?$/u;

/**
 * Permanent redirect target for a retired rubʿ URL (`/rub/N`, `/t/{lang}/{translator}/rub/N`,
 * and their `.md` twins). A rubʿ is a hizb quarter, not a reading unit with its own page: the
 * URL lands on the surah root (Arabic or translated) anchored at the quarter's first ayah. The
 * `.md` twin lands on the surah's text variant, which has no anchors. Pure, like
 * surahLocalRedirectTarget; out-of-range quarters or unknown translations yield null (404).
 */
export function rubRedirectTarget(pathname: string): SurahLocalRedirectTarget | null {
  const translated = TRANSLATED_RUB.exec(pathname);
  const arabic = translated ? null : ARABIC_RUB.exec(pathname);
  if (!translated && !arabic) return null;
  const index = Number(translated ? translated[3] : arabic![1]);
  const markdown = (translated ? translated[4] : arabic![2]) !== undefined;
  const quarter = QURAN_DATA.rangeByIndex(RangeKind.HizbQuarter, index);
  if (!quarter) return null;
  const [surahNum, ayah] = quarter.first.split(":").map(Number);
  const surah = surahNum === undefined ? undefined : QURAN_DATA.surahByNum(surahNum);
  if (!surah || ayah === undefined) return null;

  let path = `/${surah.slug}`;
  if (translated) {
    if (!translation(translated[1]!, translated[2]!)) return null;
    path = `/${surah.slug}/t/${translated[1]}/${translated[2]}`;
  }
  if (markdown) return { path: `${path}.md`, fragment: "" };
  return { path, fragment: ayah === 1 ? "" : `#ayah-${surah.num}-${ayah}` };
}

export interface TafsirRedirectTarget {
  /** Bare canonical Arabic reader path (surah root or range route). */
  readonly path: string;
  /** Tafsir catalogue id the caller appends to the ?more= stack. */
  readonly moreId: string;
}

const TRANSLATED_SURAH = /^\/([^/]+)\/t\/([^/]+)\/([^/]+)$/u;
const TRANSLATED_RANGE = /^\/t\/([^/]+)\/([^/]+)\/(page|juz|hizb)\/([1-9][0-9]*)$/u;

function tafsirSourceId(lang: string, translator: string): string | null {
  if (!CONTENT_LANGUAGE_SEGMENT.test(lang) || !TRANSLATOR_SEGMENT.test(translator)) return null;
  const sourceId = translationIdFromSegments(lang, translator);
  return isTafsirTranslation(sourceId) && TRANSLATION_BY_ID.has(sourceId) ? sourceId : null;
}

/**
 * Redirect target for a tafsir reader URL (TRX-01, EC-50): `/{surah}/t/ar/muyassar`
 * and `/t/ar/{muyassar|jalalayn}/{page|juz|hizb}/{n}` land on the Arabic reader
 * with the tafsir stacked (`?more=`). Pure, like the other redirect builders —
 * the hooks branch owns the /ar prefix, the query merge, and the fragment.
 * Non-tafsir translations, unknown slugs/sources, and bad bounds yield null
 * (caller falls through to the parse 404).
 */
export function tafsirRedirectTarget(pathname: string): TafsirRedirectTarget | null {
  const range = TRANSLATED_RANGE.exec(pathname);
  if (range) {
    const moreId = tafsirSourceId(range[1]!, range[2]!);
    const kind = rangeKindFromSegment(range[3]!);
    const index = positiveInteger(range[4]!);
    if (!moreId || !kind || index === null || !rangeIndexInRange(kind, index)) return null;
    return { path: `/${range[3]}/${range[4]}`, moreId };
  }
  const surah = TRANSLATED_SURAH.exec(pathname);
  if (surah) {
    const moreId = tafsirSourceId(surah[2]!, surah[3]!);
    if (!moreId || !QURAN_DATA.surahBySlug(surah[1]!)) return null;
    return { path: `/${surah[1]}`, moreId };
  }
  return null;
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
  // Scheme A: reader routes live directly in the (application) group — their
  // route ids are "/(application)/…" with no "/app" marker segment. Strip the
  // group prefix, drop matcher spellings ("[surah=surahSlug]" → "[surah]") and
  // the markdown twin suffix, then switch on the canonical id.
  const group = "/(application)";
  if (!routeId.startsWith(`${group}/`)) return null;
  const routePattern = routeId
    .slice(group.length)
    .replace(/\[([a-z]+)(?:=[^\]]+)?\]/giu, "[$1]")
    .replace(/\.md$/u, "");
  const surah = requiredParam(params, "surah", SURAH_SEGMENT);
  const lang = requiredParam(params, "lang", CONTENT_LANGUAGE_SEGMENT);
  const translator = requiredParam(params, "translator", TRANSLATOR_SEGMENT);
  const n = requiredParam(params, "n", POSITIVE_INTEGER);
  const slug = requiredParam(params, "slug", SNIPPET_SLUG_SEGMENT);

  switch (routePattern) {
    case "/juz":
      return parseReaderPath("/juz");
    case "/surah":
      return parseReaderPath("/surah");
    case "/pages":
      return parseReaderPath("/pages");
    case "/[surah]":
      return surah ? parseReaderPath(`/${surah}`) : null;
    case "/page/[n]":
      return n ? parseReaderPath(`/page/${n}`) : null;
    case "/juz/[n]":
      return n ? parseReaderPath(`/juz/${n}`) : null;
    case "/hizb/[n]":
      return n ? parseReaderPath(`/hizb/${n}`) : null;
    case "/azkar/[slug]":
      return slug ? parseReaderPath(`/azkar/${slug}`) : null;
    case "/[surah]/t/[lang]/[translator]":
      return surah && lang && translator
        ? parseReaderPath(`/${surah}/t/${lang}/${translator}`)
        : null;
    case "/t/[lang]/[translator]/azkar/[slug]":
      return lang && translator && slug
        ? parseReaderPath(`/t/${lang}/${translator}/azkar/${slug}`)
        : null;
    case "/t/[lang]/[translator]/page/[n]":
      return lang && translator && n ? parseReaderPath(`/t/${lang}/${translator}/page/${n}`) : null;
    case "/t/[lang]/[translator]/juz/[n]":
      return lang && translator && n ? parseReaderPath(`/t/${lang}/${translator}/juz/${n}`) : null;
    case "/t/[lang]/[translator]/hizb/[n]":
      return lang && translator && n ? parseReaderPath(`/t/${lang}/${translator}/hizb/${n}`) : null;
    default:
      return null;
  }
}
