export const prerender = true;

import { MARKETING_ROUTES } from "$lib/config/site-structure";
import { htmlToMarkdown, mdToPlain, pagePath, textVariantEntries } from "$lib/seo/render";

function matchMarketingText(slug: string): boolean {
  if (slug === "") return false;
  const page = slug === "index" ? "/" : `/${slug}`;
  // SAFETY: widening the literal route values to string[] only loosens the
  // membership test; `page` is an untrusted string param, never a caller type.
  return (Object.values(MARKETING_ROUTES) as readonly string[]).includes(page);
}

import type { RequestHandler } from "./$types";

export const entries = textVariantEntries;

export const GET: RequestHandler = async ({ fetch, params }) => {
  // Marketing-only by contract: a reader slug (.e.g /al-baqarah.txt) must 404,
  // never internally fetch the surah HTML and mint-plain-text it by accident.
  // (Prerender entries already constrain the build; this guards runtime too.)
  if (params.slug === undefined || !matchMarketingText(params.slug)) {
    return new Response("Not found", { status: 404, headers: { "content-type": "text/plain; charset=utf-8" } });
  }
  const res = await fetch(pagePath(params.slug));
  const md = htmlToMarkdown(await res.text());
  return new Response(mdToPlain(md), {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
};
