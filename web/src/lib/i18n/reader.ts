import { assertUiLocale, type UiLocale } from "$lib/i18n/locales";
import { localizeHref } from "$lib/paraglide/runtime";

export type QuranReaderHref = "/app" | `/app/${string}`;
export type LocalizedReaderHref<Locale extends UiLocale = UiLocale> = `/${Locale}/app${string}`;

const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const CONTENT_LANGUAGE_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const TRANSLATOR_SEGMENT = /^[a-z0-9]+(?:[.-][a-z0-9]+)*$/;
const POSITIVE_INTEGER_SEGMENT = /^[1-9]\d*$/;
const CANONICAL_LOCAL_PAGE_SEGMENT = /^(?:[2-9]|[1-9]\d+)$/;
const RESERVED_SURAH_SEGMENTS = new Set(["juz", "page", "t"]);

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

function splitHref(value: string): { pathname: string; suffix: string } {
  const queryIndex = value.indexOf("?");
  const fragmentIndex = value.indexOf("#");
  const suffixIndex =
    queryIndex < 0
      ? fragmentIndex
      : fragmentIndex < 0
        ? queryIndex
        : Math.min(queryIndex, fragmentIndex);

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

function isCanonicalLocalPageSegment(value: string): boolean {
  return CANONICAL_LOCAL_PAGE_SEGMENT.test(value);
}

function isReaderPathname(pathname: string): boolean {
  if (pathname === "/app") return true;
  if (!pathname.startsWith("/app/")) return false;

  const segments = pathname.slice(5).split("/");
  if (segments.some((segment) => segment === "")) return false;

  switch (segments.length) {
    case 1:
      return segments[0] === "juz" || isSurahSegment(segments[0]!);
    case 2:
      return (
        (segments[0] === "page" || segments[0] === "juz") && isPositiveIntegerSegment(segments[1]!)
      );
    case 3:
      return (
        isSurahSegment(segments[0]!) &&
        segments[1] === "page" &&
        isCanonicalLocalPageSegment(segments[2]!)
      );
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
        (segments[3] === "page" || segments[3] === "juz") &&
        isPositiveIntegerSegment(segments[4]!)
      );
    case 6:
      return (
        isSurahSegment(segments[0]!) &&
        segments[1] === "t" &&
        isContentLanguageSegment(segments[2]!) &&
        isTranslatorSegment(segments[3]!) &&
        segments[4] === "page" &&
        isCanonicalLocalPageSegment(segments[5]!)
      );
    default:
      return false;
  }
}

function isCanonicalReaderHref(value: unknown): value is QuranReaderHref {
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
  const localized = localizeHref(quranHref, { locale });
  const sourceParts = splitHref(quranHref);
  const localizedParts = splitHref(localized);
  if (
    localizedParts.pathname !== `/${locale}${sourceParts.pathname}` ||
    localizedParts.suffix !== sourceParts.suffix
  ) {
    throw new Error(`Invalid localized reader href: ${localized}`);
  }
  return localized as LocalizedReaderHref<Locale>;
}

export function readerHomeHrefFor<const Locale extends UiLocale>(locale: Locale): `/${Locale}/app` {
  assertUiLocale(locale);
  return localizeReaderHref(locale, "/app") as `/${Locale}/app`;
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
