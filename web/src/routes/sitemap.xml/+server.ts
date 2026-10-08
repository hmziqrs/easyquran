export const prerender = true;

import {
  quranHrefForPrerenderEntry,
  readerPrerenderEntries,
} from "#lib/components/i18n/reader-prerender.server.js";
import { SITE } from "#lib/config/site.js";
import type { SurahRouteContext } from "#lib/data/quran.js";
import { SUPPORTED_UI_LOCALES, type UiLocale } from "#lib/i18n/locales.js";
import {
  MARKETING_PUBLICATIONS,
  marketingHref,
  type MarketingPageId,
} from "#lib/i18n/marketing.js";
import type { QuranReaderHref } from "#lib/i18n/reader.js";
import { marketingSeoLinks, readerCanonicalEntryPath, readerCanonicalPath } from "#lib/i18n/seo.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

const XML_ENTITIES: Readonly<Record<string, string>> = Object.freeze({
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&apos;",
});

const escape = (value: string) => value.replace(/[&<>"']/g, (c) => XML_ENTITIES[c] ?? c);

const ARABIC: SurahRouteContext = { kind: "arabic" };

function readerUrl(arabicPath: QuranReaderHref): string {
  // One <loc> per content location: the unprefixed en canonical (scheme A).
  // Translations stay discovery-only (D9/D12) and never appear. The UI-locale
  // pair is the en loc + one ar alternate; en owns x-default (same policy as
  // the marketing pages, minus per-content-language alternates — D13).
  const canonical = readerCanonicalPath(arabicPath);
  return readerUrlBlock(SITE.url + canonical, SITE.url + `/ar${canonical}`);
}

/** Shared <url> block: one canonical loc + the en/ar/x-default UI triple. */
function readerUrlBlock(loc: string, arHref: string): string {
  const alternates = [
    `    <xhtml:link rel="alternate" hreflang="en" href="${escape(loc)}"/>`,
    `    <xhtml:link rel="alternate" hreflang="ar" href="${escape(arHref)}"/>`,
    `    <xhtml:link rel="alternate" hreflang="x-default" href="${escape(loc)}"/>`,
  ].join("\n");
  return `  <url>\n    <loc>${escape(loc)}</loc>\n${alternates}\n  </url>`;
}

function localizedMarketingUrl(pageId: MarketingPageId, locale: UiLocale): string {
  const links = marketingSeoLinks(pageId, locale);
  const alternates = links.alternates
    .map(
      ({ hreflang, href }) =>
        `    <xhtml:link rel="alternate" hreflang="${escape(hreflang)}" href="${escape(href)}"/>`,
    )
    .join("\n");
  return `  <url>\n    <loc>${escape(links.canonical)}</loc>\n${alternates}\n  </url>`;
}

function plainReaderEntryUrl(page: "juz-index" | "surah-index" | "pages-index"): string {
  const path = readerCanonicalEntryPath(page);
  return readerUrlBlock(SITE.url + path, SITE.url + `/ar${path}`);
}

function* sitemapLines(): Generator<string> {
  yield `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n`;
  // SAFETY: MARKETING_PUBLICATIONS is an Object.freeze literal whose keys are exactly MarketingPageId
  // (the type is derived from that object), so Object.keys enumerates that key set at runtime.
  for (const pageId of Object.keys(MARKETING_PUBLICATIONS) as MarketingPageId[]) {
    for (const locale of SUPPORTED_UI_LOCALES) {
      if (!marketingHref(pageId, locale)) continue;
      yield localizedMarketingUrl(pageId, locale);
      yield "\n";
    }
  }
  for (const entry of readerPrerenderEntries(QURAN_DATA)) {
    yield readerUrl(quranHrefForPrerenderEntry(entry, ARABIC));
    yield "\n";
  }
  // The noindex /app hub is gone (scheme A); the three bounded indexes are
  // indexable and submitted.
  yield plainReaderEntryUrl("juz-index");
  yield "\n";
  yield plainReaderEntryUrl("surah-index");
  yield "\n";
  yield plainReaderEntryUrl("pages-index");
  yield "\n</urlset>";
}

export function GET() {
  const encoder = new TextEncoder();
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of sitemapLines()) {
        controller.enqueue(encoder.encode(chunk));
      }
      controller.close();
    },
  });

  return new Response(stream, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
    },
  });
}
