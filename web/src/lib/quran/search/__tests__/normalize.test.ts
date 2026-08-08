import { describe, expect, it } from "vite-plus/test";
import { isEligibleQuery, normalizeArabic } from "$lib/quran/search/normalize";
import corpus from "$lib/quran/__fixtures__/parity.json";

describe("normalize parity (shared corpus)", () => {
  expect(corpus.normalize.length).toBeGreaterThan(0);
  for (const c of corpus.normalize) {
    it(`normalizes — ${c.rule}`, () => {
      expect(normalizeArabic(c.input)).toBe(c.expected);
    });
  }
});

describe("isEligibleQuery", () => {
  it("requires the 3-char floor for plain text", () => {
    expect(isEligibleQuery(normalizeArabic("الحمد"))).toBe(true);
    expect(isEligibleQuery(normalizeArabic("ا"))).toBe(false);
    expect(isEligibleQuery(normalizeArabic("من"))).toBe(false);
  });

  it("allows a lone Quranic ornament below the floor (quran.com parity)", () => {
    expect(isEligibleQuery(normalizeArabic("۞"))).toBe(true);
    expect(isEligibleQuery(normalizeArabic("۩"))).toBe(true);
  });
});
