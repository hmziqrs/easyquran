export const prerender = true;

import {
  quranHrefForPrerenderEntry,
  readerPrerenderEntries,
} from "$lib/components/i18n/reader-prerender.server";
import { SITE } from "$lib/config/site";
import { translationSegmentsFromId, type SurahRouteContext } from "$lib/data/quran";
import { TRANSLATIONS } from "$lib/data/translations";
import { SUPPORTED_UI_LOCALES, type UiLocale } from "$lib/i18n/locales";
import { MARKETING_PUBLICATIONS, marketingHref, type MarketingPageId } from "$lib/i18n/marketing";
import type { QuranReaderHref } from "$lib/i18n/reader";
import {
  marketingSeoLinks,
  readerCanonicalEntryPath,
  readerCanonicalPath,
  type ReaderEntryPage,
} from "$lib/i18n/seo";
import { QURAN_DATA } from "$lib/server/quran-data";

const XML_ENTITIES: Readonly<Record<string, string>> = Object.freeze({
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&apos;",
});

const escape = (value: string) => value.replace(/[&<>"']/g, (c) => XML_ENTITIES[c] ?? c);

const ARABIC: SurahRouteContext = { kind: "arabic" };

type TranslationEntry = { lang: string; ctx: SurahRouteContext };

const translations: TranslationEntry[] = TRANSLATIONS.map((translation) => {
    const { lang, translator } = translationSegmentsFromId(translation.id);
    return { lang, ctx: { kind: "translation", lang, translator } };
  });

function alternatesBlock(
  arabicLoc: string,
  pathForCtx: (ctx: SurahRouteContext) => QuranReaderHref,
): string {
  let out = `    <xhtml:link rel="alternate" hreflang="ar" href="${escape(arabicLoc)}"/>`;
  out += `\n    <xhtml:link rel="alternate" hreflang="x-default" href="${escape(arabicLoc)}"/>`;
  const seen = new Set(["ar"]);
  for (const t of translations) {
    if (seen.has(t.lang)) continue;
    seen.add(t.lang);
    const href = SITE.url + readerCanonicalPath(pathForCtx(t.ctx));
    out += `\n    <xhtml:link rel="alternate" hreflang="${escape(t.lang)}" href="${escape(href)}"/>`;
  }
  return out;
}

function groupedUrl(
  arabicPath: QuranReaderHref,
  pathForCtx: (ctx: SurahRouteContext) => QuranReaderHref,
): string {
  const loc = SITE.url + readerCanonicalPath(arabicPath);
  return `  <url>\n    <loc>${escape(loc)}</loc>\n${alternatesBlock(loc, pathForCtx)}\n  </url>`;
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

function plainReaderEntryUrl(page: ReaderEntryPage): string {
  return `  <url>\n    <loc>${escape(SITE.url + readerCanonicalEntryPath(page))}</loc>\n  </url>`;
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
    yield groupedUrl(quranHrefForPrerenderEntry(entry, ARABIC), (ctx) =>
      quranHrefForPrerenderEntry(entry, ctx),
    );
    yield "\n";
  }
  yield plainReaderEntryUrl("home");
  yield "\n";
  yield plainReaderEntryUrl("juz-index");
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
