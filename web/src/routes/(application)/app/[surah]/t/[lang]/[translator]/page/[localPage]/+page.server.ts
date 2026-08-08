import { error, redirect } from "@sveltejs/kit";
import { translationSurahPath } from "$lib/data/quran";
import { QURAN_DATA } from "$lib/server/quran-data";
import { loadTranslationSurahRouteData } from "$lib/server/quran-translation-page";
import type { PageServerLoad } from "./$types";

export const prerender = false;

type LocalPageAction =
  | { kind: "not_found" }
  | { kind: "redirect"; status: 308; destination: `/app/${string}` }
  | { kind: "proceed"; localPage: number };

function resolveLocalPageAction(raw: string, destination: `/app/${string}`): LocalPageAction {
  const localPage = Number(raw);
  if (!Number.isSafeInteger(localPage) || localPage < 1) return { kind: "not_found" };
  if (localPage === 1) return { kind: "redirect", status: 308, destination };
  return { kind: "proceed", localPage };
}

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const surah = QURAN_DATA.surahBySlug(params.surah);
  if (!surah) throw error(404, `Unknown surah: ${params.surah}`);
  const action = resolveLocalPageAction(
    params.localPage,
    translationSurahPath(surah.slug, params.lang, params.translator, 1),
  );
  if (action.kind === "not_found") throw error(404, `Unknown Surah page: ${params.localPage}`);
  if (action.kind === "redirect") throw redirect(action.status, action.destination);
  const data = await loadTranslationSurahRouteData(
    surah,
    action.localPage,
    params.lang,
    params.translator,
    fetch,
  );
  if (!data) throw error(404, `Unknown Surah page: ${params.localPage}`);
  if (data.pageData.ayahs.length === 0) {
    setHeaders({ "x-eq-translation-pending": "1", "x-robots-tag": "noindex, follow" });
  }
  return data;
};
