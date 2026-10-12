import { error } from "@sveltejs/kit";

import { SNIPPETS, snippetForSlug, type SnippetKind, type SnippetRef } from "#lib/data/snippets.js";
import { stripTajweedMarkup } from "#lib/quran/view/tajweed.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { readRangeText } from "#lib/server/quran-sqlite.js";

import type { PageServerLoad } from "./$types";

// Arabic-only prerender (Feature 7.2). The /t/{lang}/{translator}/azkar/[slug]
// twin is SSR + 7-day disk cache, never SSG — its route files sit under
// t/[lang]/[translator]/azkar/.
export const prerender = true;

export function entries() {
  return SNIPPETS.map((snippet) => ({ slug: snippet.slug }));
}

/** One verse row: key + ayah number + display text (tajweed markup stripped —
 * the editorial page renders plain text; the reader renders the markup). */
export interface SnippetAyahRow {
  key: string;
  ayah: number;
  text: string;
}

/** One snippet ref resolved to its surah metadata + verse rows. */
export interface SnippetRefGroup {
  surah: number;
  slug: string;
  name: string;
  arabicName: string;
  ayahStart: number;
  ayahEnd: number;
  ayahs: SnippetAyahRow[];
}

export interface SnippetPageData {
  slug: string;
  kind: SnippetKind;
  title: { en: string; ar: string };
  description: { en: string; ar: string };
  refs: SnippetRefGroup[];
}

function refGroup(ref: SnippetRef): SnippetRefGroup {
  const surah = QURAN_DATA.surahByNum(ref.surah);
  const start = QURAN_DATA.globalIndexOf(ref.surah, ref.ayahStart);
  const end = QURAN_DATA.globalIndexOf(ref.surah, ref.ayahEnd);
  // Curated data: a bad ref is an authoring bug, not a user 404.
  if (!surah || start === undefined || end === undefined || end < start) {
    throw error(500, `Invalid snippet ref: ${ref.surah}:${ref.ayahStart}-${ref.ayahEnd}`);
  }
  const { ayahs } = readRangeText(start, end);
  return {
    surah: surah.num,
    slug: surah.slug,
    name: surah.name,
    arabicName: surah.arabic,
    ayahStart: ref.ayahStart,
    ayahEnd: ref.ayahEnd,
    ayahs: ayahs.map((ayah) => ({
      key: ayah.key,
      ayah: ayah.ayah,
      text: stripTajweedMarkup(ayah.text),
    })),
  };
}

export const load: PageServerLoad = ({ params }) => {
  const snippet = snippetForSlug(params.slug);
  if (!snippet) throw error(404, `Unknown snippet: ${params.slug}`);
  const data: SnippetPageData = {
    slug: snippet.slug,
    kind: snippet.kind,
    title: snippet.title,
    description: snippet.description,
    refs: snippet.refs.map((ref) => refGroup(ref)),
  };
  return data;
};
