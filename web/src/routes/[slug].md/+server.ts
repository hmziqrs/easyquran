export const prerender = true;

import { htmlToMarkdown, pagePath, textVariantEntries } from "$lib/seo/render";
import type { RequestHandler } from "./$types";

export const entries = textVariantEntries;

export const GET: RequestHandler = async ({ fetch, params }) => {
  const res = await fetch(pagePath(params.slug));
  const html = await res.text();
  return new Response(htmlToMarkdown(html), {
    headers: { "content-type": "text/markdown; charset=utf-8" },
  });
};
