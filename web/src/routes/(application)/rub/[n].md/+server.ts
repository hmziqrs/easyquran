import { rubPathFor, type SurahRouteContext } from "$lib/data/quran";
import { readerHrefFor } from "$lib/i18n/reader";
import { loadRangeData } from "$lib/server/quran-range";
import { renderRangePageMarkdown } from "$lib/server/reader-markdown";
import { rangeEntries, requireRangeIndex } from "$lib/server/reader-route-guards";

import type { RequestHandler } from "./$types";

export const prerender = true;

const ARABIC: SurahRouteContext = { kind: "arabic" };

export function entries() {
  return rangeEntries("rub");
}

export const GET: RequestHandler = ({ params }) => {
  const index = requireRangeIndex("rub", params.n);
  const data = loadRangeData("rub", index);
  const body = renderRangePageMarkdown(data, readerHrefFor("en", rubPathFor(ARABIC, index)));
  return new Response(body, { headers: { "content-type": "text/markdown; charset=utf-8" } });
};
