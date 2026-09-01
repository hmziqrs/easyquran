import {
  OpenerKind,
  type Ayah,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";

import { bodyText } from "./source-view.ts";
import { stripTajweedMarkup } from "./tajweed.ts";

export function headerText(normalization: SurahNormalization): string | null {
  if (normalization.openerKind !== OpenerKind.Header) return null;
  if (normalization.openerText === null) return null;
  // The opener text feeds the bismillah SVG title attribute (plain-text view);
  // tajweed markup is view-only and stripped here — a no-op for other sources.
  return stripTajweedMarkup(normalization.openerText);
}

export function displayVerses(surah: QuranSurahText): string[] {
  return surah.verses.map((raw, index) => bodyText(raw, index + 1, surah.normalization));
}

export interface QuranRangeGroup {
  readonly surah: number;
  readonly ayahs: readonly Ayah[];
  readonly normalization: SurahNormalization;
  readonly opener: string | null;
}

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
