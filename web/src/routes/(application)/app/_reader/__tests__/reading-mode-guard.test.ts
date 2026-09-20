import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => {
  const entry = (id: string, language: string, languageCode: string) => ({
    id,
    language,
    languageCode,
    direction: "ltr" as const,
    name: `Name ${id}`,
    translator: `Translator ${id}`,
    sizeBytes: 2048,
    downloadUrl: "",
  });
  return {
    catalogue: [
      entry("en.sahih", "English", "en"),
      entry("en.arberry", "English", "en"),
      entry("ur.jalandhry", "Urdu", "ur"),
    ],
  };
});

vi.mock("$lib/quran/catalogue", () => {
  const byId = new Map(h.catalogue.map((t) => [t.id, t]));
  return { TRANSLATION_CATALOGUE_BY_ID: byId };
});

import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
import {
  readingBannerVisible,
  readingCandidates,
  readingModeHrefFor,
  readingModeUi,
} from "../reading-mode-guard.svelte";

function fixtureEntry(i: number): TranslationCatalogueEntry {
  // SAFETY: the fixture mirrors the real catalogue entry shape; the assertion only satisfies noUncheckedIndexedAccess on the fixture array.
  return h.catalogue[i] as TranslationCatalogueEntry;
}

describe("readingCandidates", () => {
  it("returns no candidates for an Arabic primary with zero extras (no confirmation)", () => {
    expect(readingCandidates(null, [])).toEqual([]);
  });

  it("offers only the translation primary when there are no extras (single-candidate confirm)", () => {
    const out = readingCandidates("en.sahih", []);
    // SAFETY: index 0 exists by fixture construction, and the fixture mirrors the catalogue entry shape.
    expect(out).toEqual([{ id: "en.sahih", entry: fixtureEntry(0) }]);
  });

  it("offers the Arabic current source first, then each stacked extra", () => {
    const out = readingCandidates(null, ["en.sahih", "ur.jalandhry"]);
    expect(out.map((c) => c.id)).toEqual([null, "en.sahih", "ur.jalandhry"]);
  });

  it("offers the route primary first and excludes it from extras", () => {
    const out = readingCandidates("en.sahih", ["en.sahih", "en.arberry", "ur.jalandhry"]);
    expect(out.map((c) => c.id)).toEqual(["en.sahih", "en.arberry", "ur.jalandhry"]);
  });

  it("drops stacked ids unknown to the catalogue", () => {
    const out = readingCandidates("en.sahih", ["nope.unknown", "en.arberry"]);
    expect(out.map((c) => c.id)).toEqual(["en.sahih", "en.arberry"]);
  });
});

describe("readingModeHrefFor", () => {
  it("returns null for the Arabic candidate and the current route primary (no navigation)", () => {
    expect(readingModeHrefFor({ id: null, entry: null }, "en.sahih", "/app/al-fatihah")).toBeNull();
    expect(
      readingModeHrefFor({ id: "en.sahih", entry: fixtureEntry(0) }, "en.sahih", "/app/al-fatihah"),
    ).toBeNull();
  });

  it("builds a position-preserving translation href carrying ?mode=reading", () => {
    const out = readingModeHrefFor(
      { id: "en.arberry", entry: fixtureEntry(1) },
      "ms.basmeih",
      "/app/t/ms/basmeih/juz/30",
    );
    expect(out).toBe("/app/t/en/arberry/juz/30?mode=reading");
  });

  it("preserves surah local pages when switching primaries", () => {
    const out = readingModeHrefFor(
      { id: "ur.jalandhry", entry: fixtureEntry(2) },
      null,
      "/app/al-baqarah/page/3",
    );
    expect(out).toBe("/app/al-baqarah/t/ur/jalandhry/page/3?mode=reading");
  });
});

describe("readingBannerVisible (URL-initiated fallback only)", () => {
  beforeEach(() => {
    readingModeUi.reset();
  });

  it("shows for a URL-initiated transition with hidden extras", () => {
    expect(readingBannerVisible({ reading: true, hiddenCount: 2, dismissed: false })).toBe(true);
  });

  it("hides once a UI switch marked the reading session", () => {
    readingModeUi.mark();
    expect(readingBannerVisible({ reading: true, hiddenCount: 2, dismissed: false })).toBe(false);
  });

  it("stays hidden after dismissal or with nothing hidden", () => {
    expect(readingBannerVisible({ reading: true, hiddenCount: 2, dismissed: true })).toBe(false);
    expect(readingBannerVisible({ reading: true, hiddenCount: 0, dismissed: false })).toBe(false);
    expect(readingBannerVisible({ reading: false, hiddenCount: 2, dismissed: false })).toBe(false);
  });
});
