import { describe, expect, it, vi } from "vite-plus/test";
import {
  LANGUAGE_FLAGS,
  TRANSLATION_CATALOGUE,
  flagFor,
} from "$lib/quran/catalogue";
import { TRANSLATIONS } from "$lib/data/translations";

vi.mock("$lib/config/site", () => ({
  QURAN: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [],
  },
}));

describe("language flag map", () => {
  it("has an explicit entry for every languageCode in the baked catalogue", () => {
    const codes = new Set(TRANSLATIONS.map((t) => t.languageCode));
    for (const code of codes) {
      expect(LANGUAGE_FLAGS[code], `missing flag entry for ${code}`).toBeDefined();
    }
  });

  it("catalogue rows resolve a flag without hitting the fallback", () => {
    for (const entry of TRANSLATION_CATALOGUE) {
      expect(LANGUAGE_FLAGS[entry.languageCode]).toBeDefined();
    }
  });

  it("falls back to the globe emoji for unknown codes", () => {
    expect(flagFor("xx-unknown").flag).toBe("\u{1F310}");
    expect(flagFor("xx-unknown").country).toBe("");
  });

  it("carries a country name for country-backed languages", () => {
    expect(flagFor("en")).toEqual({ flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" });
    expect(flagFor("ur").country).toBe("Pakistan");
    expect(flagFor("tr").country).toBe("Turkey");
    expect(flagFor("ku").flag).toBe("\u{1F310}");
  });
});
