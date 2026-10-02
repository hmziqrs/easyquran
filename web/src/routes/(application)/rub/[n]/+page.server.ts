import { loadRangeData } from "$lib/server/quran-range";
import { rangeEntries, requireRangeIndex } from "$lib/server/reader-route-guards";

import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return rangeEntries("rub");
}

export const load: PageServerLoad = ({ params }) =>
  loadRangeData("rub", requireRangeIndex("rub", params.n));
