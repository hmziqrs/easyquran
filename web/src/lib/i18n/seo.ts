import { SITE } from "$lib/config/site";
import { SUPPORTED_UI_LOCALES, type UiLocale } from "$lib/i18n/locales";
import { marketingHref, type MarketingPageId } from "$lib/i18n/marketing";
import { readerHomeHrefFor, readerHrefFor, type QuranReaderHref } from "$lib/i18n/reader";
import type { PublicHref } from "$lib/i18n/public-href";

export const READER_CANONICAL_UI_LOCALE = "en" as const satisfies UiLocale;

export interface SeoAlternate {
  hreflang: string;
  href: string;
}

export interface MarketingSeoLinks {
  canonical: string;
  alternates: readonly SeoAlternate[];
}

export type ReaderEntryPage = "home" | "juz-index";

function absoluteHref(path: string): string {
  return `${SITE.url}${path}`;
}

/**
 * Returns only published UI-locale variants. English owns x-default.
 */
export function marketingSeoLinks(
  pageId: MarketingPageId,
  currentLocale: UiLocale,
): MarketingSeoLinks {
  const currentHref = marketingHref(pageId, currentLocale);
  if (!currentHref) {
    throw new Error(`[i18n-seo] unpublished marketing page: ${pageId}/${currentLocale}`);
  }

  const alternates = SUPPORTED_UI_LOCALES.flatMap((locale) => {
    const href = marketingHref(pageId, locale);
    return href ? [{ hreflang: locale, href: absoluteHref(href) }] : [];
  });
  const englishHref = marketingHref(pageId, "en");
  if (!englishHref) {
    throw new Error(`[i18n-seo] missing English publication: ${pageId}`);
  }

  return {
    canonical: absoluteHref(currentHref),
    alternates: [...alternates, { hreflang: "x-default", href: absoluteHref(englishHref) }],
  };
}

/**
 * Reader UI variants share one SEO canonical: English UI. Quran source
 * segments are already present in quranHref and remain untouched.
 */
export function readerCanonicalPath(quranHref: QuranReaderHref): string {
  return readerHrefFor(READER_CANONICAL_UI_LOCALE, quranHref);
}

export function readerCanonicalUrl(quranHref: QuranReaderHref): string {
  return absoluteHref(readerCanonicalPath(quranHref));
}

/** Bounded reader indexes sit outside Quran-content route descriptors. */
export function readerEntryPath(locale: UiLocale, page: ReaderEntryPage): PublicHref {
  return page === "home" ? readerHomeHrefFor(locale) : readerHrefFor(locale, "/app/juz");
}

export function readerCanonicalEntryPath(page: ReaderEntryPage): PublicHref {
  return readerEntryPath(READER_CANONICAL_UI_LOCALE, page);
}
