import type { TranslationDirection } from "$lib/data/quran-types";

/** One stacked translation for the demo surah, read straight from its local sqlite. */
export interface MixTranslation {
  readonly id: string;
  /** Catalogue display name — native script for some languages (Urdu: جالندہری). */
  readonly name: string;
  readonly translator: string;
  readonly language: string;
  readonly languageCode: string;
  readonly direction: TranslationDirection;
  /** Ayah text by position: texts[0] is ayah 1. */
  readonly texts: readonly string[];
}

export interface MixSurahRow {
  readonly num: number;
  readonly name: string;
  readonly arabic: string;
  readonly meaning: string;
  readonly meta: string;
}

/** A juz or mushaf page in the browse drawer. */
export interface MixRangeRow {
  readonly index: number;
  readonly first: string;
  readonly last: string;
  readonly startName: string;
  readonly endName: string;
  /** First words of the range's opening ayah (opener stripped), for the Arabic list. */
  readonly opening: string;
  /** Juz the range starts in (for a juz row: itself). */
  readonly juz: number;
  /** Mushaf pages the range covers (1 for a page row). */
  readonly pages: number;
}

export interface MixSurah {
  readonly num: number;
  readonly name: string;
  readonly arabic: string;
  readonly meaning: string;
  readonly meta: string;
  readonly pageCount: number;
  /** Surah tile hue slot (1–4), same cycle the live header band uses. */
  readonly hue: 1 | 2 | 3 | 4;
}

export interface MixData {
  readonly surah: MixSurah;
  readonly opener: string | null;
  readonly verses: readonly string[];
  readonly translations: readonly MixTranslation[];
  /** Requested ids with no local sqlite (or an unknown id). */
  readonly missing: readonly string[];
  readonly surahs: readonly MixSurahRow[];
  readonly juz: readonly MixRangeRow[];
  readonly pages: readonly MixRangeRow[];
  readonly activeJuz: number;
  readonly activePage: number;
}
