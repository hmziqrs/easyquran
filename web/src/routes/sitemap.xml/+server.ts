export const prerender = true;

import { SITE, MARKETING_PAGES } from "$lib/config/site";

const escape = (value: string) => value.replace(/&/g, "&amp;");

export function GET() {
  // Public pages only — the /app product UI is intentionally absent.
  const urls = MARKETING_PAGES.map(
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
