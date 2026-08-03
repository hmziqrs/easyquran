import { asset } from "$app/paths";
import { createQuranData, RangeKind } from "$lib/data/quran-data";
import type { PageLoad } from "./$types";

export const prerender = true;

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(asset("/quran-meta/quran-data.json"), {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[juz index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const ajzur = quran.ranges(RangeKind.Juz).map((juz) => ({
    index: juz.index,
    first: juz.first,
    last: juz.last,
  }));
  return { ajzur };
};
