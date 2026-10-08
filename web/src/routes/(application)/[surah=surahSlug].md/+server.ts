import { error } from "@sveltejs/kit";

import { surahPathFor, type SurahRouteContext } from "#lib/data/quran.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { readSurahLocalPageData } from "#lib/server/quran-surah-page.js";
import { renderSurahPageMarkdown } from "#lib/server/reader-markdown.js";
import { requireSurah } from "#lib/server/reader-route-guards.js";

import type { RequestHandler } from "./$types";

export const prerender = true;

const ARABIC: SurahRouteContext = { kind: "arabic" };

export function entries() {
  return QURAN_DATA.surahs.map((s) => ({ surah: s.slug }));
}

export const GET: RequestHandler = ({ params }) => {
  const surah = requireSurah(params.surah);
  const pageData = readSurahLocalPageData(surah, 1);
  if (!pageData) throw error(404, `Unknown surah: ${params.surah}`);
  const body = renderSurahPageMarkdown(pageData, readerHrefFor("en", surahPathFor(ARABIC, surah)));
  return new Response(body, { headers: { "content-type": "text/markdown; charset=utf-8" } });
};
