import { describe, expect, it } from "vite-plus/test";
import {
  surahLocalPagePath,
  translationGlobalPagePath,
  translationIdFromSegments,
  translationJuzPath,
  translationSegmentsFromId,
  translationSurahPath,
} from "$lib/data/quran";

const ROUTE_LITERALS = ["t", "page", "juz"] as const;

function segmentAfterSlug(path: string, slug: string): string | undefined {
  const prefix = `/app/${slug}/`;
  if (!path.startsWith(prefix)) return undefined;
  return path.slice(prefix.length).split("/")[0];
}

describe("translation route grammar — non-shadow", () => {
  it("uses 't' (not 'page') directly after a surah slug, so the two trees cannot collide", () => {
    const slug = "al-baqarah";
    const arabicLocal = surahLocalPagePath(slug, 3);
    const translationLocal = translationSurahPath(slug, "en", "sahih", 1);

    expect(arabicLocal).toBe("/app/al-baqarah/page/3");
    expect(translationLocal).toBe("/app/al-baqarah/t/en/sahih");

    const arabicSeg = segmentAfterSlug(arabicLocal, slug);
    const translationSeg = segmentAfterSlug(translationLocal, slug);
    expect(arabicSeg).toBe("page");
    expect(translationSeg).toBe("t");
    expect(arabicSeg).not.toBe(translationSeg);
  });

  it("collapses translation local page 1 to the bare translation route", () => {
    const one = translationSurahPath("al-baqarah", "en", "sahih", 1);
    const two = translationSurahPath("al-baqarah", "en", "sahih", 2);
    expect(one).toBe("/app/al-baqarah/t/en/sahih");
    expect(two).toBe("/app/al-baqarah/t/en/sahih/page/2");
    expect(one.endsWith("/page/1")).toBe(false);
  });

  it("places global translation routes under the literal /app/t/ tree", () => {
    const globalPage = translationGlobalPagePath("en", "sahih", 42);
    const juz = translationJuzPath("en", "sahih", 30);
    expect(globalPage).toBe("/app/t/en/sahih/page/42");
    expect(juz).toBe("/app/t/en/sahih/juz/30");
    expect(globalPage.startsWith("/app/t/")).toBe(true);
    expect(juz.startsWith("/app/t/")).toBe(true);
  });

  it("keeps t, page, and juz mutually distinct as route literals", () => {
    expect(new Set(ROUTE_LITERALS).size).toBe(ROUTE_LITERALS.length);
    for (const a of ROUTE_LITERALS) {
      for (const b of ROUTE_LITERALS) {
        if (a !== b) expect(a).not.toBe(b);
      }
    }
  });
});

describe("translation id <-> path segments round-trip", () => {
  it("round-trips a plain lang.translator id", () => {
    expect(translationIdFromSegments("en", "sahih")).toBe("en.sahih");
    expect(translationSegmentsFromId("en.sahih")).toEqual({ lang: "en", translator: "sahih" });
  });

  it("round-trips a translator whose own name contains dots", () => {
    const lang = "en";
    const translator = "sahih.int";
    const id = translationIdFromSegments(lang, translator);
    expect(id).toBe("en.sahih.int");
    const back = translationSegmentsFromId(id);
    expect(back).toEqual({ lang, translator });
    expect(translationIdFromSegments(back.lang, back.translator)).toBe(id);
  });

  it("round-trips across several dotted translator names", () => {
    for (const translator of ["sahih", "sahih.int", "khan.maududi", "a.b.c"]) {
      const id = translationIdFromSegments("en", translator);
      const back = translationSegmentsFromId(id);
      expect(back).toEqual({ lang: "en", translator });
      expect(translationIdFromSegments(back.lang, back.translator)).toBe(id);
    }
  });
});
