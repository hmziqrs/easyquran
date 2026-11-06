export const prerender = true;

import { getLocale } from "#lib/paraglide/runtime.js";
import { htmlToMarkdown, textVariantEntries, textVariantSource } from "#lib/seo/render.js";

import type { RequestHandler } from "./$types";

export const entries = textVariantEntries;

export const GET: RequestHandler = async ({ fetch, params }) => {
  // The request locale comes from the `/ar` prefix (rerouted here); the source page
  // is the Arabic twin when one is published, so the variant body matches its URL.
  const source = textVariantSource(params.slug, getLocale());
  const res = await fetch(source.path);
  const html = await res.text();
  return new Response(htmlToMarkdown(html), {
    headers: {
      "content-type": "text/markdown; charset=utf-8",
      "content-language": source.lang,
    },
  });
};
