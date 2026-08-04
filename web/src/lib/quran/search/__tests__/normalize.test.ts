import { describe, expect, it } from "vite-plus/test";
import { normalizeArabic } from "$lib/quran/search/normalize";
import corpus from "$lib/quran/__fixtures__/parity.json";

describe("normalize parity (shared corpus)", () => {
  expect(corpus.normalize.length).toBeGreaterThan(0);
  for (const c of corpus.normalize) {
    it(`normalizes — ${c.rule}`, () => {
      expect(normalizeArabic(c.input)).toBe(c.expected);
    });
  }
});
