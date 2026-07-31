/* ════════════════════════════════════════════════════════════════════════
   verses.ts — display prep for the reader variants.

   For the 112 surahs whose `bismillah` is "embedded-prefix", the Tanzil
   Uthmani text begins ayah 1 with the basmala itself. A reader that ALSO
   renders the standalone basmala header therefore shows it twice — which is
   what the shipping SurahReader currently does (see the note in the handover:
   worth fixing there too). Printed mushafs set the basmala as its own line and
   start ayah 1 at the words after it, so that's what the variants do.

   The two spellings are NOT byte-identical, in two separate ways:
     • the verse text writes ٱلرَّحْمَـٰنِ with a tatweel (U+0640) carrying the
       superscript alef, the constant writes it without;
     • the harakat sit in a different order — the constant has fatha then
       shadda (U+064E U+0651), the verse text shadda then fatha.
   So the comparison runs over a SKELETON: the string with every combining
   mark, quranic annotation sign and tatweel removed, leaving base letters and
   spaces. The cut point is then mapped back onto the ORIGINAL string, so not
   one character of the retained text is rewritten. If the skeleton doesn't
   match, the verse is returned verbatim — never guess at Quranic text.
   ════════════════════════════════════════════════════════════════════════ */

import { BISMILLAH } from "$lib/data/quran";
import type { Surah } from "$lib/data/quran";

/** Harakat + tanwin (064B–065F), tatweel (0640), superscript alef (0670)
 *  and the quranic annotation signs (06D6–06ED). Written as \u escapes so the
 *  class can't silently swallow the Arabic-Indic digits at 0660–0669, which sit
 *  between two of these ranges. */
const MARK_CLASS = "[\\u064B-\\u065F\\u0640\\u0670\\u06D6-\\u06ED]";
/** Single-character test (stateless \u2014 no /g, so `lastIndex` can't drift). */
const MARK = new RegExp(MARK_CLASS);
const MARK_ALL = new RegExp(MARK_CLASS, "g");

const skeleton = (s: string): string => s.replace(MARK_ALL, "");

/** Ayah 1 with a leading basmala removed, or unchanged if there isn't one. */
export function withoutBasmalaPrefix(verse: string): string {
  const target = skeleton(BISMILLAH);
  if (!skeleton(verse).startsWith(target)) return verse;

  // Walk the original, counting only base characters, so the cut index lands
  // correctly however the marks are spelled or ordered.
  let seen = 0;
  let i = 0;
  for (; i < verse.length && seen < target.length; i++) {
    if (!MARK.test(verse[i]!)) seen++;
  }
  // …then take the marks hanging off that final base letter with it.
  while (i < verse.length && MARK.test(verse[i]!)) i++;
  return verse.slice(i).trimStart();
}

/** The surah's verses as they should be DISPLAYED beneath a basmala header. */
export function displayVerses(surah: Surah): string[] {
  if (surah.bismillah !== "embedded-prefix") return surah.verses;
  const [first, ...rest] = surah.verses;
  return first === undefined ? surah.verses : [withoutBasmalaPrefix(first), ...rest];
}
