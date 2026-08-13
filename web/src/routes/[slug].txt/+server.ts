export const prerender = true;

import { htmlToMarkdown, mdToPlain, pagePath, textVariantEntries } from "$lib/seo/render";

import type { RequestHandler } from "./$types";

export const entries = textVariantEntries;

export const GET: RequestHandler = async ({ fetch, params }) => {
  const res = await fetch(pagePath(params.slug));
  const md = htmlToMarkdown(await res.text());
  return new Response(mdToPlain(md), {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
};
