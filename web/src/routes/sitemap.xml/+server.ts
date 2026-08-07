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

const escape = (value: string) => value.replace(/&/g, "&amp;");
const ARABIC: SurahRouteContext = { kind: "arabic" };

export function GET() {
  const marketing = MARKETING_PAGES.map(
    (page) => `  <url>\n    <loc>${escape(SITE.url + page.href)}</loc>\n  </url>`,
  ).join("\n");
  const surahs = QURAN_DATA.surahs
    .map((s) => `  <url>\n    <loc>${escape(SITE.url + surahPathFor(ARABIC, s))}</loc>\n  </url>`)
    .join("\n");
  const surahPages = QURAN_DATA.surahs
    .flatMap((surah) =>
      Array.from({ length: QURAN_DATA.surahLocalPageCount(surah.num) - 1 }, (_, index) => {
        const href = surahLocalPagePathFor(ARABIC, surah, index + 2);
        return `  <url>\n    <loc>${escape(SITE.url + href)}</loc>\n  </url>`;
      }),
    )
    .join("\n");
  const globalPages = Array.from(
    { length: 604 },
    (_, i) =>
      `  <url>\n    <loc>${escape(SITE.url + globalPagePathFor(ARABIC, i + 1))}</loc>\n  </url>`,
  ).join("\n");
  const juz = Array.from(
    { length: 30 },
    (_, i) => `  <url>\n    <loc>${escape(SITE.url + juzPathFor(ARABIC, i + 1))}</loc>\n  </url>`,
  ).join("\n");
  const translations = (rawTranslations as readonly (readonly string[])[])
    .flatMap((row) => {
      const { lang, translator } = translationSegmentsFromId(row[0]);
      const ctx: SurahRouteContext = { kind: "translation", lang, translator };
      const homes = QURAN_DATA.surahs.map(
        (s) => `  <url>\n    <loc>${escape(SITE.url + surahPathFor(ctx, s))}</loc>\n  </url>`,
      );
      const translatedSurahPages = QURAN_DATA.surahs.flatMap((surah) =>
        Array.from({ length: QURAN_DATA.surahLocalPageCount(surah.num) - 1 }, (_, index) => {
          const href = surahLocalPagePathFor(ctx, surah, index + 2);
          return `  <url>\n    <loc>${escape(SITE.url + href)}</loc>\n  </url>`;
        }),
      );
      const translatedGlobalPages = Array.from(
        { length: 604 },
        (_, i) =>
          `  <url>\n    <loc>${escape(SITE.url + globalPagePathFor(ctx, i + 1))}</loc>\n  </url>`,
      );
      const translatedJuz = Array.from(
        { length: 30 },
        (_, i) => `  <url>\n    <loc>${escape(SITE.url + juzPathFor(ctx, i + 1))}</loc>\n  </url>`,
      );
      return [...homes, ...translatedSurahPages, ...translatedGlobalPages, ...translatedJuz];
    })
    .join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${marketing}
${surahs}
${surahPages}
${globalPages}
${juz}
${translations}
</urlset>`;

  return new Response(xml, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
    },
  });
}
