import { describe, expect, it, vi } from "vite-plus/test";
import { LANGUAGE_AUTONYMS, TRANSLATION_CATALOGUE, nativeNameFor } from "$lib/quran/catalogue";
import { TRANSLATIONS } from "$lib/data/translations";

vi.mock("$lib/config/site", () => ({
  QURAN: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [],
  },
}));

describe("language autonym map", () => {
  it("resolves every languageCode in the baked catalogue (autonym or documented null fallback)", () => {
    const codes = new Set(TRANSLATIONS.map((t) => t.languageCode));
    for (const code of codes) {
      expect(
        LANGUAGE_AUTONYMS[code],
        `missing autonym entry for ${code} (add the native name or an explicit null fallback)`,
      ).toBeDefined();
    }
    // every non-null autonym is a usable, non-empty string
    for (const [code, autonym] of Object.entries(LANGUAGE_AUTONYMS)) {
      if (autonym === null) continue;
      expect(autonym.trim().length, `empty autonym for ${code}`).toBeGreaterThan(0);
    }
  });

  it("non-null autonyms differ from the catalogue's English language name", () => {
    // guards against lazy copies of the English name posing as autonyms.
    // First-seen name per code: some codes carry two English variants across
    // sources (mdh = "Maguindanaon"/"Magindanawn") and the autonym
    // legitimately matches one of them.
    const englishNameByCode = new Map<string, string>();
    for (const t of TRANSLATION_CATALOGUE) {
      if (!englishNameByCode.has(t.languageCode)) {
        englishNameByCode.set(t.languageCode, t.language);
      }
    }
    for (const [code, autonym] of Object.entries(LANGUAGE_AUTONYMS)) {
      if (autonym === null) continue;
      const english = englishNameByCode.get(code);
      if (english === undefined) continue;
      expect(autonym.trim().toLowerCase(), `autonym for ${code} duplicates the English name`).not.toBe(
        english.trim().toLowerCase(),
      );
    }
  });

  it("spot-checks autonyms including RTL and CJK scripts", () => {
    expect(nativeNameFor("ar")).toBe("العربية");
    expect(nativeNameFor("ur")).toBe("اردو");
    expect(nativeNameFor("fa")).toBe("فارسی");
    expect(nativeNameFor("hi")).toBe("हिन्दी");
    expect(nativeNameFor("fr")).toBe("Français");
    expect(nativeNameFor("de")).toBe("Deutsch");
    expect(nativeNameFor("zh")).toBe("中文");
    expect(nativeNameFor("ja")).toBe("日本語");
    expect(nativeNameFor("id")).toBe("Bahasa Indonesia");
    expect(nativeNameFor("tr")).toBe("Türkçe");
    expect(nativeNameFor("bn")).toBe("বাংলা");
    expect(nativeNameFor("ru")).toBe("Русский");
  });

  it("falls back to null (render the English name) where no distinct autonym exists", () => {
    // English/Hausa/Tagalog: the native name IS the English name
    expect(nativeNameFor("en")).toBeNull();
    expect(nativeNameFor("ha")).toBeNull();
    expect(nativeNameFor("tl")).toBeNull();
  });

  it("unknown codes resolve to the null fallback instead of guessing", () => {
    expect(nativeNameFor("xx-unknown")).toBeNull();
  });
});
