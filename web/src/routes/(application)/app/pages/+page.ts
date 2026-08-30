import { asset } from "$app/paths";
import { createQuranData, RangeKind } from "$lib/data/quran-data";

import type { PageLoad } from "./$types";

export const prerender = true;

export interface PageIndexRow {
  index: number;
  /** First verse key on the mushaf page, e.g. "2:142". */
  first: string;
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(asset("/quran-meta/quran-data.json"), {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[pages index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const rows: PageIndexRow[] = quran.ranges(RangeKind.Page).map((page) => ({
    index: page.index,
    first: page.first,
  }));
  return { rows };
};
