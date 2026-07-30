export const prerender = true;

import { SITE, MARKETING_PAGES } from "$lib/config/site";
import { CATALOG } from "$lib/data/quran-meta";

const escape = (value: string) => value.replace(/&/g, "&amp;");

export function GET() {
  // Marketing pages + the 114 indexable surah reader pages (doc §5 SEO cutover).
  // The /app index itself is a redirect and stays out.
  const marketing = MARKETING_PAGES.map(
    (page) => `  <url>\n    <loc>${escape(SITE.url + page.href)}</loc>\n  </url>`,
  ).join("\n");
  const surahs = CATALOG.map(
    (s) => `  <url>\n    <loc>${escape(SITE.url + "/app/" + s.slug)}</loc>\n  </url>`,
  ).join("\n");
  const juz = Array.from(
    { length: 30 },
    (_, i) => `  <url>\n    <loc>${escape(SITE.url + "/app/juz/" + (i + 1))}</loc>\n  </url>`,
  ).join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${marketing}
${surahs}
${juz}
</urlset>`;

  return new Response(xml, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
    },
  });
}
