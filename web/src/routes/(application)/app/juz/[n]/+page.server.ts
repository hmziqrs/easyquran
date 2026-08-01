import { error } from "@sveltejs/kit";
import { loadRangeData } from "$lib/server/quran-range";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return Array.from({ length: 30 }, (_, i) => ({ n: String(i + 1) }));
}

export const load: PageServerLoad = ({ params }) => {
  const index = Number(params.n);
  if (!Number.isInteger(index) || index < 1 || index > 30) {
    throw error(404, `Unknown juz: ${params.n}`);
  }
  return loadRangeData("juz", index);
};
