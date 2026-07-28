export const prerender = true;

import { SITE, NAV_PAGES } from "$lib/config/site";

const escape = (value: string) => value.replace(/&/g, "&amp;");

export function GET() {
  const urls = NAV_PAGES.map(
    (page) => `  <url>\n    <loc>${escape(SITE.url + page.href)}</loc>\n  </url>`,
  ).join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>`;

  return new Response(xml, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
    },
  });
}
