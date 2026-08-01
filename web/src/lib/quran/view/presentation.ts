import { OpenerKind, type QuranSurahText, type SurahNormalization } from "$lib/data/quran-types";
import { bodyText } from "./source-view.ts";

/** Exact source opener when it is a separate canonical header. */
export function headerText(normalization: SurahNormalization): string | null {
  return normalization.openerKind === OpenerKind.Header ? normalization.openerText : null;
}

/** Raw verses projected to canonical numbered bodies for reader display. */
export function displayVerses(surah: QuranSurahText): string[] {
  return surah.verses.map((raw, index) => bodyText(raw, index + 1, surah.normalization));
}
