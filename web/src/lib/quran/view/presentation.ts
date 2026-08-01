import {
  OpenerKind,
  type Ayah,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";
import { bodyText } from "./source-view.ts";

/** Exact source opener when it is a separate canonical header. */
export function headerText(normalization: SurahNormalization): string | null {
  return normalization.openerKind === OpenerKind.Header ? normalization.openerText : null;
}

/** Raw verses projected to canonical numbered bodies for reader display. */
export function displayVerses(surah: QuranSurahText): string[] {
  return surah.verses.map((raw, index) => bodyText(raw, index + 1, surah.normalization));
}

export interface QuranRangeGroup {
  readonly surah: number;
  readonly ayahs: readonly Ayah[];
  readonly normalization: SurahNormalization;
  readonly opener: string | null;
}

/**
 * Group a canonical range for presentation. A header is emitted only when the
 * range actually contains ayah 1, so page/juz SSG and any future SPA range
 * loader share the same boundary semantics.
 */
export function groupRangeAyahs(
  ayahs: readonly Ayah[],
  normalizations: readonly SurahNormalization[],
): QuranRangeGroup[] {
  const descriptors = new Map(normalizations.map((value) => [value.surah, value]));
  const groups: {
    surah: number;
    ayahs: Ayah[];
    normalization: SurahNormalization;
    opener: string | null;
  }[] = [];

  for (const ayah of ayahs) {
    let group = groups.at(-1);
    if (!group || group.surah !== ayah.surah) {
      const normalization = descriptors.get(ayah.surah);
      if (!normalization) {
        throw new Error(`Missing Quran normalization for surah ${ayah.surah}`);
      }
      group = {
        surah: ayah.surah,
        ayahs: [],
        normalization,
        opener: ayah.ayah === 1 ? headerText(normalization) : null,
      };
      groups.push(group);
    }
    group.ayahs.push(ayah);
  }
  return groups;
}
