import { detectEmbeddedPrefix, scalarSlice } from "$lib/quran/view/source-view";
import { describe, expect, it } from "vite-plus/test";

const REFERENCE = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ";
const MULK_1 = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ تَبَـٰرَكَ ٱلَّذِى بِيَدِهِ ٱلْمُلْكُ وَهُوَ عَلَىٰ كُلِّ شَىْءٍ قَدِيرٌ";

describe("canonical embedded-prefix partition", () => {
  it("returns scalar cuts and retains the body as a verbatim suffix", () => {
    const cut = detectEmbeddedPrefix(MULK_1, REFERENCE);
    expect(cut).not.toBeNull();
    const body = scalarSlice(MULK_1, cut!.bodyStartScalar);
    expect(body.startsWith("تَبَـٰرَكَ")).toBe(true);
    expect(MULK_1.endsWith(body)).toBe(true);
  });

  it("does not guess when the source-local skeleton is absent", () => {
    expect(detectEmbeddedPrefix("ٱلْحَمْدُ لِلَّهِ", REFERENCE)).toBeNull();
  });

  it("uses the source's own simple-clean reference", () => {
    const raw = "بسم الله الرحمن الرحيم الم";
    expect(detectEmbeddedPrefix(raw, "بسم الله الرحمن الرحيم")).toEqual({
      openerEndScalar: 22,
      bodyStartScalar: 23,
    });
  });
});
