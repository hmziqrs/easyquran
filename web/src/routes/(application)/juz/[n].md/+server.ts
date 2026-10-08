import { juzPathFor, type SurahRouteContext } from "#lib/data/quran.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { loadRangeData } from "#lib/server/quran-range.js";
import { renderRangePageMarkdown } from "#lib/server/reader-markdown.js";
import { rangeEntries, requireRangeIndex } from "#lib/server/reader-route-guards.js";

import type { RequestHandler } from "./$types";

export const prerender = true;

const ARABIC: SurahRouteContext = { kind: "arabic" };

export function entries() {
  return rangeEntries("juz");
}

export const GET: RequestHandler = ({ params }) => {
  const index = requireRangeIndex("juz", params.n);
  const data = loadRangeData("juz", index);
  const body = renderRangePageMarkdown(data, readerHrefFor("en", juzPathFor(ARABIC, index)));
  return new Response(body, { headers: { "content-type": "text/markdown; charset=utf-8" } });
};
