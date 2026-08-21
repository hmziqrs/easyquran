import { TRANSLATION_BY_ARTIFACT_PATH, TRANSLATION_BY_ID } from "$lib/data/translations";
import {
  peekTranslationName,
  TRANSLATION_CATALOGUE,
  TRANSLATION_CATALOGUE_BY_ID,
} from "$lib/quran/catalogue";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({
  QURAN: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [],
  },
}));

describe("baked translation catalogue", () => {
  it("contains complete same-origin metadata without fetching", () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const sqNahi = TRANSLATION_CATALOGUE_BY_ID.get("sq.nahi");

    expect(TRANSLATION_CATALOGUE.length).toBeGreaterThan(100);
    expect(sqNahi).toEqual({
      id: "sq.nahi",
      language: "Albanian",
      languageCode: "sq",
      direction: "ltr",
      name: "Efendi Nahi",
      translator: "Hasan Efendi Nahi",
      sizeBytes: 1175552,
      downloadUrl: "/_quran/tanzil/translations/sqlite/sq.nahi.sqlite",
    });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("provides stable indexed metadata and labels", () => {
    const byId = TRANSLATION_BY_ID.get("sq.nahi");
    const byPath = TRANSLATION_BY_ARTIFACT_PATH.get(
      "tanzil/translations/sqlite/sq.nahi.sqlite",
    );

    expect(byId).toBe(byPath);
    expect(peekTranslationName("sq.nahi")).toBe("Efendi Nahi");
    expect(peekTranslationName("xx.missing")).toBeNull();
  });

  it("exports frozen catalogue records", () => {
    expect(Object.isFrozen(TRANSLATION_CATALOGUE)).toBe(true);
    expect(Object.isFrozen(TRANSLATION_CATALOGUE[0])).toBe(true);
  });
});
