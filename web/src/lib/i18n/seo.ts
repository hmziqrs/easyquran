import { SITE } from "#lib/config/site.js";
import { SUPPORTED_UI_LOCALES, type UiLocale } from "#lib/i18n/locales.js";
import { marketingHref, type MarketingPageId } from "#lib/i18n/marketing.js";
import type { PublicHref } from "#lib/i18n/public-href.js";
import { readerHrefFor, type QuranReaderHref } from "#lib/i18n/reader.js";

export interface SeoAlternate {
  hreflang: string;
  href: string;
}

export interface MarketingSeoLinks {
  canonical: string;
  alternates: readonly SeoAlternate[];
}

export type ReaderEntryPage = "juz-index" | "surah-index" | "pages-index";

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
 * Reader UI variants share one SEO canonical: the unprefixed en UI (scheme A).
 * The canonical path is the validated input itself — `en` hrefs are unprefixed,
 * so readerHrefFor("en", …) is identity and the canonical equals quranHref.
 */
export function readerCanonicalPath(quranHref: QuranReaderHref): string {
  return readerHrefFor("en", quranHref);
}

export function readerCanonicalUrl(quranHref: QuranReaderHref): string {
  return absoluteHref(readerCanonicalPath(quranHref));
}

const ENTRY_PATHS: Readonly<Record<ReaderEntryPage, QuranReaderHref>> = Object.freeze({
  "juz-index": "/juz",
  "surah-index": "/surah",
  "pages-index": "/pages",
});

/** Bounded reader indexes sit outside Quran-content route descriptors. */
export function readerEntryPath(locale: UiLocale, page: ReaderEntryPage): PublicHref {
  return readerHrefFor(locale, ENTRY_PATHS[page]);
}

export function readerCanonicalEntryPath(page: ReaderEntryPage): PublicHref {
  return readerEntryPath("en", page);
}
