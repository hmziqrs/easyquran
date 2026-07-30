// SSG load for /app/page/[n]. Prerenders all 604 Hafs/Madani pages, reading each
// page's ayah range from quran-uthmani.sqlite at build (no backend, offline-first).
import { error } from "@sveltejs/kit";
import { loadRangeData } from "$lib/server/quran-range";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return Array.from({ length: 604 }, (_, i) => ({ n: String(i + 1) }));
}

export const load: PageServerLoad = ({ params }) => {
  const index = Number(params.n);
  if (!Number.isInteger(index) || index < 1 || index > 604) {
    throw error(404, `Unknown page: ${params.n}`);
  }
  return loadRangeData("page", index);
};
