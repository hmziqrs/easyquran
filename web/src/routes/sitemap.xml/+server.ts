export const prerender = true;

import { SITE, MARKETING_PAGES } from "$lib/config/site";
import {
  globalPagePathFor,
  juzPathFor,
  surahLocalPagePathFor,
  surahPathFor,
  translationSegmentsFromId,
  type SurahRouteContext,
} from "$lib/data/quran";
import { QURAN_DATA } from "$lib/server/quran-data";
import rawTranslations from "$lib/data/translations.json";

const escape = (value: string) =>
  value.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&apos;",
  );

const ARABIC: SurahRouteContext = { kind: "arabic" };

type TranslationEntry = { lang: string; ctx: SurahRouteContext };

const translations: TranslationEntry[] = (rawTranslations as readonly (readonly string[])[]).map(
  (row) => {
    const { lang, translator } = translationSegmentsFromId(row[0]);
    return { lang, ctx: { kind: "translation", lang, translator } };
  },
);

function alternatesBlock(arabicLoc: string, pathForCtx: (ctx: SurahRouteContext) => string): string {
  let out = `    <xhtml:link rel="alternate" hreflang="ar" href="${escape(arabicLoc)}"/>`;
  out += `\n    <xhtml:link rel="alternate" hreflang="x-default" href="${escape(arabicLoc)}"/>`;
  const seen = new Set(["ar"]);
  for (const t of translations) {
    if (seen.has(t.lang)) continue;
    seen.add(t.lang);
    const href = SITE.url + pathForCtx(t.ctx);
    out += `\n    <xhtml:link rel="alternate" hreflang="${escape(t.lang)}" href="${escape(href)}"/>`;
  }
  return out;
}

function groupedUrl(arabicPath: string, pathForCtx: (ctx: SurahRouteContext) => string): string {
  const loc = SITE.url + arabicPath;
  return `  <url>\n    <loc>${escape(loc)}</loc>\n${alternatesBlock(loc, pathForCtx)}\n  </url>`;
}

function plainUrl(path: string): string {
  return `  <url>\n    <loc>${escape(SITE.url + path)}</loc>\n  </url>`;
}

function* sitemapLines(): Generator<string> {
  yield `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n`;
  for (const page of MARKETING_PAGES) {
    yield plainUrl(page.href);
    yield "\n";
  }
  for (const s of QURAN_DATA.surahs) {
    yield groupedUrl(surahPathFor(ARABIC, s), (ctx) => surahPathFor(ctx, s));
    yield "\n";
  }
  for (const s of QURAN_DATA.surahs) {
    const pageCount = QURAN_DATA.surahLocalPageCount(s.num);
    for (let localPage = 2; localPage <= pageCount; localPage += 1) {
      yield groupedUrl(surahLocalPagePathFor(ARABIC, s, localPage), (ctx) =>
        surahLocalPagePathFor(ctx, s, localPage),
      );
      yield "\n";
    }
  }
  for (let i = 1; i <= 604; i += 1) {
    yield groupedUrl(globalPagePathFor(ARABIC, i), (ctx) => globalPagePathFor(ctx, i));
    yield "\n";
  }
  for (let i = 1; i <= 30; i += 1) {
    yield groupedUrl(juzPathFor(ARABIC, i), (ctx) => juzPathFor(ctx, i));
    yield "\n";
  }
  yield plainUrl("/app/juz");
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
