import { assertUiLocale, type UiLocale } from "#lib/i18n/locales.js";
import { localizeHref } from "#lib/paraglide/runtime.js";

/** Validated unprefixed reader href (en canonical). `isCanonicalReaderHref`
 * enforces the path grammar at the public boundary; the type is deliberately
 * loose (`/${string}`) because the grammar lives in `isReaderPathname`. */
export type QuranReaderHref = `/${string}`;
export type LocalizedReaderHref<Locale extends UiLocale = UiLocale> = Locale extends "ar"
  ? `/ar${string}`
  : QuranReaderHref;

const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const CONTENT_LANGUAGE_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
// Keep in lockstep with the TRANSLATOR_SEGMENT copies in src/hooks.ts, src/lib/accept-parse.ts,
// and src/lib/server/reader-route.ts (driven by the baked ids in src/lib/data/translations.json,
// which include underscore translator segments like quranenc.en.hilali_khan and qul r158.*).
const TRANSLATOR_SEGMENT = /^[a-z0-9]+(?:[._-][a-z0-9]+)*$/;
const POSITIVE_INTEGER_SEGMENT = /^[1-9]\d*$/;
const RANGE_SEGMENT_KINDS = new Set(["page", "juz", "hizb", "rub"]);
/** Site-root reserved words (scheme A): these single segments are reader range
 * kinds, the `t` translation marker, or app index/personal routes — never a
 * surah slug. This is what keeps /t, /page, /juz, /yours, … from being
 * swallowed by the dynamic [surah] route at the site root. */
const RESERVED_SURAH_SEGMENTS = new Set([
  // reader range kinds, the translation marker, and reader-owned indexes
  "juz",
  "page",
  "hizb",
  "rub",
  "t",
  "surah",
  "pages",
  "yours",
  // bounded product pages
  "search",
  "settings",
  "bookmarks",
  // static top-level routes (marketing, auth, account, ops) — they outrank the
  // dynamic [surah] route, and a canonical reader builder must never emit them
  "about",
  "faq",
  "contact",
  "privacy",
  "terms",
  "login",
  "register",
  "forgot-password",
  "verify-email",
  "account",
  "auth",
  "health",
  "design",
]);
/** Single-segment reader indexes (/juz, /surah, /pages). /yours and /bookmarks are
 * personal app routes — never localized, never in this set. */
const READER_INDEX_SEGMENTS = new Set(["juz", "surah", "pages"]);

function hasUnsafeUrlCharacter(value: string): boolean {
  for (const character of value) {
    const codePoint = character.codePointAt(0)!;
    if (codePoint <= 0x20 || codePoint === 0x7f || character === "\\") return true;
  }
  return false;
}

function hasValidPercentEncoding(value: string): boolean {
  for (let index = value.indexOf("%"); index >= 0; index = value.indexOf("%", index + 1)) {
    if (!/^[0-9a-f]{2}$/i.test(value.slice(index + 1, index + 3))) return false;
  }
  return true;
}

function hasEncodedControlCharacter(value: string): boolean {
  for (let index = value.indexOf("%"); index >= 0; index = value.indexOf("%", index + 3)) {
    const byte = Number.parseInt(value.slice(index + 1, index + 3), 16);
    if (byte <= 0x1f || byte === 0x7f) return true;
  }
  return false;
}

/** Earliest of two `indexOf` results, where -1 means "not found". -1 only if both are absent. */
function earliestIndex(a: number, b: number): number {
  if (a < 0) return b;
  if (b < 0) return a;
  return Math.min(a, b);
}

function splitHref(value: string) {
  const queryIndex = value.indexOf("?");
  const fragmentIndex = value.indexOf("#");
  const suffixIndex = earliestIndex(queryIndex, fragmentIndex);

  if (suffixIndex < 0) return { pathname: value, suffix: "" };
  return { pathname: value.slice(0, suffixIndex), suffix: value.slice(suffixIndex) };
}

function hasEmptyQueryOrFragment(value: string): boolean {
  const { suffix } = splitHref(value);
  if (suffix === "") return false;
  if (suffix === "?" || suffix === "#" || suffix === "?#") return true;

  if (suffix.startsWith("?")) {
    const fragmentIndex = suffix.indexOf("#");
    if (fragmentIndex === 1) return true;
    if (fragmentIndex >= 0 && fragmentIndex === suffix.length - 1) return true;
  }
  return false;
}

function isSurahSegment(value: string): boolean {
  return SURAH_SEGMENT.test(value) && !RESERVED_SURAH_SEGMENTS.has(value);
}

function isContentLanguageSegment(value: string): boolean {
  return CONTENT_LANGUAGE_SEGMENT.test(value);
}

function isTranslatorSegment(value: string): boolean {
  return TRANSLATOR_SEGMENT.test(value);
}

function isPositiveIntegerSegment(value: string): boolean {
  return POSITIVE_INTEGER_SEGMENT.test(value);
}

function isRangeSegmentKind(value: string): boolean {
  return RANGE_SEGMENT_KINDS.has(value);
}

function isReaderPathname(pathname: string): boolean {
  if (!pathname.startsWith("/") || pathname === "/") return false;

  const segments = pathname.slice(1).split("/");
  if (segments.some((segment) => segment === "")) return false;

  switch (segments.length) {
    case 1:
      return READER_INDEX_SEGMENTS.has(segments[0]!) || isSurahSegment(segments[0]!);
    case 2:
      return isRangeSegmentKind(segments[0]!) && isPositiveIntegerSegment(segments[1]!);
    case 4:
      return (
        isSurahSegment(segments[0]!) &&
        segments[1] === "t" &&
        isContentLanguageSegment(segments[2]!) &&
        isTranslatorSegment(segments[3]!)
      );
    case 5:
      return (
        segments[0] === "t" &&
        isContentLanguageSegment(segments[1]!) &&
        isTranslatorSegment(segments[2]!) &&
        isRangeSegmentKind(segments[3]!) &&
        isPositiveIntegerSegment(segments[4]!)
      );
    default:
      return false;
  }
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- runtime type guard at the public href boundary; callers may pass untrusted values (tests feed null/42/String objects), so it must accept opaque input and reject non-strings itself.
function isCanonicalReaderHref(value: unknown): value is QuranReaderHref {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- deliberate non-string rejection so caller-supplied garbage throws TypeError instead of reaching the string scanners below.
  if (typeof value !== "string" || value === "") return false;
  if (
    hasUnsafeUrlCharacter(value) ||
    !hasValidPercentEncoding(value) ||
    hasEncodedControlCharacter(value) ||
    hasEmptyQueryOrFragment(value)
  ) {
    return false;
  }
  if (value.indexOf("#") !== value.lastIndexOf("#")) return false;

  const { pathname } = splitHref(value);
  return isReaderPathname(pathname);
}

function localizeReaderHref<const Locale extends UiLocale>(
  locale: Locale,
  quranHref: QuranReaderHref,
): LocalizedReaderHref<Locale> {
  if (locale === "en") {
    // SAFETY: en is the unprefixed base locale — the validated canonical href
    // IS the localized href, which is exactly the LocalizedReaderHref<"en"> shape.
    return quranHref as LocalizedReaderHref<Locale>;
  }
  const localized = localizeHref(quranHref, { locale });
  const sourceParts = splitHref(quranHref);
  const localizedParts = splitHref(localized);
  if (
    localizedParts.pathname !== `/${locale}${sourceParts.pathname}` ||
    localizedParts.suffix !== sourceParts.suffix
  ) {
    throw new Error(`Invalid localized reader href: ${localized}`);
  }
  // SAFETY: the equality check above proved `localized` is byte-identical to `/${locale}` + the canonical source pathname + its exact suffix, which is the LocalizedReaderHref<Locale> shape.
  return localized as LocalizedReaderHref<Locale>;
}

/**
 * Canonical href of the /bookmarks page for nav/footer links. Deliberately
 * unprefixed — this is the canonical URL; /ar/bookmarks is a rerouted twin
 * served with Arabic chrome (hooks.ts reroute), never a separate address.
 * Matches the /settings and /search precedent (Nav.svelte, palette sources).
 */
export function bookmarksPageHref(): "/bookmarks" {
  return "/bookmarks";
}

/**
 * Canonical href of the /yours hub. Like /bookmarks, it is a personal app
 * route addressed only by its canonical URL — /ar/yours reroutes onto it
 * (hooks.ts), and readerHrefFor rejects it so a localized link cannot ship.
 */
export function yoursPageHref(): "/yours" {
  return "/yours";
}

export function readerHrefFor<const Locale extends UiLocale>(
  locale: Locale,
  quranHref: string,
): LocalizedReaderHref<Locale> {
  assertUiLocale(locale);
  if (!isCanonicalReaderHref(quranHref)) {
    throw new TypeError(`Invalid canonical reader href: ${String(quranHref)}`);
  }
  return localizeReaderHref(locale, quranHref);
}
