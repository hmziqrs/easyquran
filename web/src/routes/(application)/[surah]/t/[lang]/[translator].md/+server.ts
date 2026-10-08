import { error } from "@sveltejs/kit";

import { translationSurahPath } from "#lib/data/quran.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { loadTranslationSurahRouteData } from "#lib/server/quran-translation-page.js";
import { renderSurahPageMarkdown } from "#lib/server/reader-markdown.js";
import { requireSurah } from "#lib/server/reader-route-guards.js";

import type { RequestHandler } from "./$types";

export const prerender = false;

export const GET: RequestHandler = async ({ params, fetch }) => {
  const surah = requireSurah(params.surah);
  const data = await loadTranslationSurahRouteData(surah, 1, params.lang, params.translator, fetch);
  if (!data) throw error(404, `Unknown Surah page: 1`);
  const canonical = translationSurahPath(surah.slug, params.lang, params.translator);
  const body = renderSurahPageMarkdown(data.pageData, readerHrefFor("en", canonical));
  return new Response(body, { headers: { "content-type": "text/markdown; charset=utf-8" } });
};
