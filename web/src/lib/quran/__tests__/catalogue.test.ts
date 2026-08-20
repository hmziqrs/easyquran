import type { SourceCatalogueEntry } from "$lib/data/quran-types";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/config/site", () => ({
  QURAN: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [],
  },
}));

import {
  bakedTranslationCatalogue,
  findCatalogueEntry,
  translationCatalogue,
} from "$lib/quran/catalogue";

describe("baked translation catalogue", () => {
  it("contains complete same-origin metadata without fetching", () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const entries = bakedTranslationCatalogue();
    const sqNahi = findCatalogueEntry(entries, "sq.nahi");

    expect(entries.length).toBeGreaterThan(100);
    expect(sqNahi).toEqual({
      kind: "translation",
      entry: {
        id: "sq.nahi",
        language: "Albanian",
        languageCode: "sq",
        direction: "ltr",
        name: "Efendi Nahi",
        translator: "Hasan Efendi Nahi",
        sizeBytes: 1175552,
        downloadUrl: "/_quran/tanzil/translations/sqlite/sq.nahi.sqlite",
      },
    });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("returns undefined for an unregistered source", () => {
    expect(findCatalogueEntry(bakedTranslationCatalogue(), "xx.missing")).toBeUndefined();
  });
});

describe("translationCatalogue", () => {
  it("extracts only translation entries from a mixed list", () => {
    const entries: SourceCatalogueEntry[] = [
      {
        kind: "arabic",
        spec: { id: "uthmani", sizeBytes: 1, downloadUrl: "/_quran/uthmani.sqlite" },
      },
      {
        kind: "translation",
        entry: {
          id: "x",
          language: "X",
          languageCode: "x",
          direction: "ltr",
          name: "X",
          translator: null,
          sizeBytes: 1,
          downloadUrl: "/_quran/x.sqlite",
        },
      },
    ];

    const translations = translationCatalogue(entries);
    expect(translations).toHaveLength(1);
    expect(translations[0]!.id).toBe("x");
  });
});
