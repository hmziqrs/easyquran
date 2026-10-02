import { loadRangeData } from "$lib/server/quran-range";
import { rangeEntries, requireRangeIndex } from "$lib/server/reader-route-guards";

import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return rangeEntries("hizb");
}

export const load: PageServerLoad = ({ params }) =>
  loadRangeData("hizb", requireRangeIndex("hizb", params.n));
