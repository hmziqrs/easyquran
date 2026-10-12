import { error } from "@sveltejs/kit";

import { snippetForSlug, type Snippet, type SnippetRef } from "#lib/data/snippets.js";
import { stripTajweedMarkup } from "#lib/quran/view/tajweed.js";
import {
  loadTranslationSnippetData,
  type TranslationSnippetGroup,
} from "#lib/server/quran-translation-page.js";
import { markTranslationPending } from "#lib/server/reader-route-guards.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { readRangeText } from "#lib/server/quran-sqlite.js";

import type { PageServerLoad } from "./$types";

// Translated snippet detail (§11.6, decided): SSR + 7-day disk cache like
// every other /t/** route — never SSG (delivery divergence, AGENTS.md).
export const prerender = false;

/** One verse row: Arabic (tajweed markup stripped for the editorial layout)
 * plus the translation line when the source carries this verse. */
export interface SnippetVerseRow {
  key: string;
  ayah: number;
  arabic: string;
  translation: string | null;
}

export interface TranslatedSnippetRefGroup {
  surah: number;
  slug: string;
  name: string;
  arabicName: string;
  ayahStart: number;
  ayahEnd: number;
  rows: SnippetVerseRow[];
}

export interface TranslatedSnippetPageData {
  snippet: Snippet;
  translationName: string;
  /** Catalogue language code + declared direction (dir never guessed, TRX-11 rule). */
  languageCode: string;
  direction: "rtl" | "ltr";
  groups: TranslatedSnippetRefGroup[];
  pending: boolean;
}

function refGroup(ref: SnippetRef, group: TranslationSnippetGroup): TranslatedSnippetRefGroup {
  const surah = QURAN_DATA.surahByNum(ref.surah);
  const start = QURAN_DATA.globalIndexOf(ref.surah, ref.ayahStart);
  const end = QURAN_DATA.globalIndexOf(ref.surah, ref.ayahEnd);
  // Curated data: a bad ref is an authoring bug, not a user 404.
  if (!surah || start === undefined || end === undefined || end < start) {
    throw error(500, `Invalid snippet ref: ${ref.surah}:${ref.ayahStart}-${ref.ayahEnd}`);
  }
  const byKey = new Map(group.ayahs.map((ayah) => [ayah.key, ayah.text]));
  const rows = readRangeText(start, end).ayahs.map((ayah) => ({
    key: ayah.key,
    ayah: ayah.ayah,
    arabic: stripTajweedMarkup(ayah.text),
    translation: byKey.get(ayah.key) ?? null,
  }));
  return {
    surah: surah.num,
    slug: surah.slug,
    name: surah.name,
    arabicName: surah.arabic,
    ayahStart: ref.ayahStart,
    ayahEnd: ref.ayahEnd,
    rows,
  };
}

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const snippet = snippetForSlug(params.slug ?? "");
  if (!snippet) throw error(404, `Unknown snippet: ${params.slug}`);
  const data = await loadTranslationSnippetData(
    snippet.refs,
    params.lang ?? "",
    params.translator ?? "",
    fetch,
  );
  const groups = snippet.refs.map((ref, index) => refGroup(ref, data.groups[index]!));
  // One degraded group makes the whole page pending: the guard marks when the
  // flat ayah list is empty, so feed it an empty list on any per-group miss.
  const pending = data.groups.some((group) => group.ayahs.length === 0);
  markTranslationPending(setHeaders, pending ? [] : (data.groups[0]?.ayahs ?? []));
  const page: TranslatedSnippetPageData = {
    snippet,
    translationName: data.translationName,
    languageCode: data.languageCode,
    direction: data.direction,
    groups,
    pending,
  };
  return page;
};
