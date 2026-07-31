/* Guards the one place the design gallery touches Quranic text: stripping the
   embedded basmala prefix off ayah 1. The risk being tested for is silent
   over-trimming — a helper that removes too much would quietly mutilate the
   verse, and nothing else in the pipeline would catch it. */

import { describe, expect, it } from "vite-plus/test";
import { BISMILLAH } from "$lib/data/quran";
import { withoutBasmalaPrefix } from "../verses";

/** Al-Mulk 67:1 exactly as it comes out of quran-uthmani.sqlite — note the
 *  tatweel in ٱلرَّحْمَـٰنِ and the shadda-before-fatha ordering, both of which
 *  differ from the BISMILLAH constant. */
const MULK_1 = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ تَبَـٰرَكَ ٱلَّذِى بِيَدِهِ ٱلْمُلْكُ وَهُوَ عَلَىٰ كُلِّ شَىْءٍ قَدِيرٌ";

describe("withoutBasmalaPrefix", () => {
  it("removes the basmala despite differing tatweel and harakat order", () => {
    const out = withoutBasmalaPrefix(MULK_1);
    expect(out.startsWith("تَبَـٰرَكَ")).toBe(true);
    expect(out.endsWith("قَدِيرٌ")).toBe(true);
  });

  it("removes nothing else — the remainder is a verbatim suffix of the source", () => {
    const out = withoutBasmalaPrefix(MULK_1);
    expect(MULK_1.endsWith(out)).toBe(true);
  });

  it("leaves a verse that does not open with the basmala untouched", () => {
    const other = "ٱلْحَمْدُ لِلَّهِ رَبِّ ٱلْعَـٰلَمِينَ";
    expect(withoutBasmalaPrefix(other)).toBe(other);
  });

  it("handles the constant's own spelling as a prefix", () => {
    expect(withoutBasmalaPrefix(`${BISMILLAH} تَبَارَكَ`)).toBe("تَبَارَكَ");
  });

  it("returns an empty string when the verse IS only the basmala", () => {
    expect(withoutBasmalaPrefix(BISMILLAH)).toBe("");
  });
});
