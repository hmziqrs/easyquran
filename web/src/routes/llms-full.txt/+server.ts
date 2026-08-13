export const prerender = true;

import { MARKETING_PAGES } from "$lib/config/site-structure";
import { htmlToMarkdown } from "$lib/seo/render";

import type { RequestHandler } from "./$types";

export const GET: RequestHandler = async ({ fetch }) => {
  const parts = await Promise.all(
    MARKETING_PAGES.map(async (p) => {
      const res = await fetch(p.href);
      return htmlToMarkdown(await res.text());
    }),
  );
  return new Response(parts.join("\n\n---\n\n") + "\n", {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
};
