export const prerender = true;

import { SITE, MARKETING_PAGES } from "$lib/config/site";
import { surahLocalPagePath } from "$lib/data/quran";
import { QURAN_DATA } from "$lib/server/quran-data";

const escape = (value: string) => value.replace(/&/g, "&amp;");

export function GET() {
  const marketing = MARKETING_PAGES.map(
    (page) => `  <url>\n    <loc>${escape(SITE.url + page.href)}</loc>\n  </url>`,
  ).join("\n");
  const surahs = QURAN_DATA.surahs
    .map((s) => `  <url>\n    <loc>${escape(SITE.url + "/app/" + s.slug)}</loc>\n  </url>`)
    .join("\n");
  const surahPages = QURAN_DATA.surahs
    .flatMap((surah) =>
      Array.from({ length: QURAN_DATA.surahLocalPageCount(surah.num) - 1 }, (_, index) => {
        const href = surahLocalPagePath(surah, index + 2);
        return `  <url>\n    <loc>${escape(SITE.url + href)}</loc>\n  </url>`;
      }),
    )
    .join("\n");
  const juz = Array.from(
    { length: 30 },
    (_, i) => `  <url>\n    <loc>${escape(SITE.url + "/app/juz/" + (i + 1))}</loc>\n  </url>`,
  ).join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${marketing}
${surahs}
${surahPages}
${juz}
</urlset>`;

  return new Response(xml, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
    },
  });
}
