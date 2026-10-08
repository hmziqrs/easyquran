import { describe, expect, it } from "vite-plus/test";

import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  resumeCtxFor,
  rubPathFor,
  surahAyahPath,
  surahAyahPathFor,
  surahPath,
  surahPathFor,
  surahRouteContext,
  translationGlobalPagePath,
  translationIdFromSegments,
  translationJuzPath,
  translationSegmentsFromId,
  translationSurahPath,
} from "#lib/data/quran.js";

const ROUTE_LITERALS = ["t", "page", "juz", "hizb", "rub"] as const;

function segmentAfterSlug(path: string, slug: string): string | undefined {
  const prefix = `/${slug}/`;
  if (!path.startsWith(prefix)) return undefined;
  return path.slice(prefix.length).split("/")[0];
}

describe("translation route grammar — non-shadow", () => {
  it("uses 't' (not a range literal) directly after a surah slug, so the two trees cannot collide", () => {
    const slug = "al-baqarah";
    const arabicRoot = surahPath(slug);
    const translationRoot = translationSurahPath(slug, "en", "sahih");

    expect(arabicRoot).toBe("/al-baqarah");
    expect(translationRoot).toBe("/al-baqarah/t/en/sahih");

    const arabicSeg = segmentAfterSlug(arabicRoot, slug);
    const translationSeg = segmentAfterSlug(translationRoot, slug);
    expect(arabicSeg).toBeUndefined();
    expect(translationSeg).toBe("t");
  });

  it("surah URLs never carry a page tail — one URL per surah", () => {
    expect(surahPath("al-baqarah").includes("/page/")).toBe(false);
    expect(translationSurahPath("al-baqarah", "en", "sahih").includes("/page/")).toBe(false);
  });

  it("places global translation routes under the literal /t/ tree", () => {
    const globalPage = translationGlobalPagePath("en", "sahih", 42);
    const juz = translationJuzPath("en", "sahih", 30);
    expect(globalPage).toBe("/t/en/sahih/page/42");
    expect(juz).toBe("/t/en/sahih/juz/30");
    expect(globalPage.startsWith("/t/")).toBe(true);
    expect(juz.startsWith("/t/")).toBe(true);
  });

  it("keeps t and every range literal mutually distinct", () => {
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

describe("surahRouteContext classifies source ids", () => {
  it("classifies an arabic source id as arabic", () => {
    expect(surahRouteContext("uthmani")).toEqual({ kind: "arabic" });
    expect(surahRouteContext("simple-clean")).toEqual({ kind: "arabic" });
  });

  it("classifies a lang.translator id as translation with lang/translator", () => {
    expect(surahRouteContext("ms.basmeih")).toEqual({
      kind: "translation",
      lang: "ms",
      translator: "basmeih",
    });
    expect(surahRouteContext("en.sahih.int")).toEqual({
      kind: "translation",
      lang: "en",
      translator: "sahih.int",
    });
  });
});

describe("route-aware path builders preserve translation across surah boundaries", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const nextSurah = "luqman";
  const prevSurah = "al-ankabut";

  it("cross-surah Next from a translated surah keeps t/<lang>/<translator> (ar-rum -> luqman)", () => {
    const next = surahPathFor(ctx, nextSurah);
    expect(next).toBe(`/${nextSurah}/t/ms/basmeih`);
    expect(next.endsWith("/page/1")).toBe(false);
  });

  it("cross-surah Previous from a translated surah keeps t/<lang>/<translator> (ar-rum -> al-ankabut)", () => {
    const prev = surahPathFor(ctx, prevSurah);
    expect(prev).toBe(`/${prevSurah}/t/ms/basmeih`);
    expect(prev.endsWith("/page/1")).toBe(false);
  });
});

describe("route-aware ayah path preserves translation (reveal, sidebar, search, continue reading)", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const surah = { slug: "ar-rum", num: 30 };

  it("surahAyahPathFor anchors the bare surah root — no page tail anywhere", () => {
    const path = surahAyahPathFor(ctx, surah, 12);
    expect(path).toBe("/ar-rum/t/ms/basmeih#ayah-30-12");
    expect(path.includes("/t/ms/basmeih")).toBe(true);
    expect(path.includes("/page/")).toBe(false);
  });

  it("surahAyahPathFor diverges from the Arabic-only surahAyahPath on a translation route", () => {
    expect(surahAyahPathFor(ctx, surah, 12)).not.toBe(surahAyahPath(surah, 12));
    expect(surahAyahPath(surah, 12)).toBe("/ar-rum#ayah-30-12");
  });
});

describe("canonical surah path for a translation source is not the Arabic path", () => {
  const ctx = surahRouteContext("ms.basmeih");

  it("surahPathFor emits the translated canonical, not the Arabic one", () => {
    const canonical = surahPathFor(ctx, "ar-rum");
    expect(canonical).toBe("/ar-rum/t/ms/basmeih");
    expect(canonical).not.toBe(surahPath("ar-rum"));
    expect(canonical.includes("/t/ms/basmeih")).toBe(true);
  });
});

describe("arabic source context stays parity with legacy helpers", () => {
  const ctx = surahRouteContext("uthmani");
  const surah = { slug: "al-baqarah", num: 2 };

  it("surahPathFor / surahAyahPathFor match the Arabic-only builders", () => {
    expect(surahPathFor(ctx, "al-baqarah")).toBe(surahPath("al-baqarah"));
    expect(surahAyahPathFor(ctx, surah, 5)).toBe(surahAyahPath(surah, 5));
  });

  it("never introduces a /t/ segment for an arabic source", () => {
    expect(surahPathFor(ctx, "al-baqarah").includes("/t/")).toBe(false);
    expect(surahAyahPathFor(ctx, surah, 5).includes("/t/")).toBe(false);
  });
});

describe("legacy Arabic-only helpers drop the translation segment (regression guard)", () => {
  it("surahAyahPath never emits a /t/ segment", () => {
    expect(surahAyahPath({ slug: "ar-rum", num: 30 }, 12)).toBe("/ar-rum#ayah-30-12");
    expect(surahAyahPath({ slug: "ar-rum", num: 30 }, 12).includes("/t/")).toBe(false);
  });

  it("surahPath never emits a /t/ segment", () => {
    expect(surahPath("luqman").includes("/t/")).toBe(false);
  });
});

describe("sitemap emits every indexable route class (canonical-seo guard)", () => {
  const ARABIC = surahRouteContext("uthmani");
  const ctx = surahRouteContext("ms.basmeih");

  it("emits the arabic global page route /page/[n] via globalPagePathFor", () => {
    expect(globalPagePathFor(ARABIC, 42)).toBe("/page/42");
    expect(globalPagePathFor(ARABIC, 1)).toBe("/page/1");
  });

  it("emits the translated global page route /t/<lang>/<translator>/page/[n] via globalPagePathFor", () => {
    expect(globalPagePathFor(ctx, 42)).toBe("/t/ms/basmeih/page/42");
    expect(globalPagePathFor(ctx, 42).includes("/t/ms/basmeih/")).toBe(true);
  });

  it("emits both arabic and translated hizb/rub routes, kept distinct", () => {
    expect(hizbPathFor(ARABIC, 60)).toBe("/hizb/60");
    expect(hizbPathFor(ctx, 60)).toBe("/t/ms/basmeih/hizb/60");
    expect(rubPathFor(ARABIC, 240)).toBe("/rub/240");
    expect(rubPathFor(ctx, 240)).toBe("/t/ms/basmeih/rub/240");
    expect(rubPathFor(ctx, 4)).not.toBe(rubPathFor(ARABIC, 4));
  });

  it("translation route classes are not silently collapsed to their arabic counterparts", () => {
    expect(globalPagePathFor(ctx, 42)).not.toBe(globalPagePathFor(ARABIC, 42));
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
  });

  it("emits both arabic and translation juz routes, kept distinct", () => {
    expect(juzPathFor(ARABIC, 30)).toBe("/juz/30");
    expect(juzPathFor(ctx, 30)).toBe("/t/ms/basmeih/juz/30");
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
  });
});

describe("reader entry redirect honors the persisted translation preference (secondary-surface guard)", () => {
  const surah = { slug: "al-baqarah", num: 2 };

  it("surahPathFor(surahRouteContext(sourceId), surah) keeps the persisted translation segment", () => {
    const ctx = surahRouteContext("ms.basmeih");
    expect(surahPathFor(ctx, surah)).toBe("/al-baqarah/t/ms/basmeih");
    expect(surahPathFor(ctx, surah).includes("/t/ms/basmeih")).toBe(true);
  });

  it("the redirect path diverges from the buggy arabic-only surahPath(surah)", () => {
    const ctx = surahRouteContext("ms.basmeih");
    expect(surahPath(surah)).toBe("/al-baqarah");
    expect(surahPathFor(ctx, surah)).not.toBe(surahPath(surah));
  });

  it("with no persisted sourceId the redirect falls back to the arabic surah path", () => {
    const ctx = { kind: "arabic" as const };
    expect(surahPathFor(ctx, surah)).toBe(surahPath(surah));
    expect(surahPathFor(ctx, surah).includes("/t/")).toBe(false);
  });
});

describe("continueReading resume uses the last-read verse's own source, not the current route", () => {
  const currentRouteArabic = surahRouteContext("uthmani");
  const currentRouteTranslation = surahRouteContext("ms.basmeih");

  type LastRead = { num: number; n: number; sourceId?: string };

  it("resume into a translation source keeps /t/<lang>/<translator> even when the current route is arabic", () => {
    const lastRead: LastRead = { num: 31, n: 4, sourceId: "ms.basmeih" };
    const surah = { slug: "luqman", num: 31 };
    const resume = surahAyahPathFor(resumeCtxFor(lastRead, currentRouteArabic), surah, lastRead.n);
    expect(resume).toBe("/luqman/t/ms/basmeih#ayah-31-4");
    expect(resume.includes("/t/ms/basmeih")).toBe(true);
  });

  it("resume into an arabic source drops /t/ even when the current route is a translation", () => {
    const lastRead: LastRead = { num: 30, n: 12, sourceId: "uthmani" };
    const surah = { slug: "ar-rum", num: 30 };
    const resume = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteTranslation),
      surah,
      lastRead.n,
    );
    expect(resume).toBe("/ar-rum#ayah-30-12");
    expect(resume.includes("/t/")).toBe(false);
  });

  it("the resume URL diverges from what the current routeContext alone would have produced", () => {
    const lastRead: LastRead = { num: 31, n: 4, sourceId: "ms.basmeih" };
    const surah = { slug: "luqman", num: 31 };
    const buggy = surahAyahPathFor(currentRouteArabic, surah, lastRead.n);
    const fixed = surahAyahPathFor(resumeCtxFor(lastRead, currentRouteArabic), surah, lastRead.n);
    expect(fixed).not.toBe(buggy);
    expect(buggy.includes("/t/")).toBe(false);
    expect(fixed.includes("/t/ms/basmeih")).toBe(true);
  });

  it("falls back to the current routeContext when lastRead carries no sourceId (backwards compat)", () => {
    const lastRead: LastRead = { num: 30, n: 12 };
    const surah = { slug: "ar-rum", num: 30 };
    const ctx = resumeCtxFor(lastRead, currentRouteTranslation);
    expect(ctx).toBe(currentRouteTranslation);
    const resume = surahAyahPathFor(ctx, surah, lastRead.n);
    expect(resume).toBe("/ar-rum/t/ms/basmeih#ayah-30-12");
  });
});

describe("translated range-route canonical is helper-built, not page.url.pathname (canonical-seo)", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const ARABIC = surahRouteContext("uthmani");

  it("globalPagePathFor emits a slash-less canonical that a trailing-slash request pathname would diverge from", () => {
    const canonical = globalPagePathFor(ctx, 7);
    expect(canonical).toBe("/t/ms/basmeih/page/7");
    expect(canonical.endsWith("/")).toBe(false);

    const trailingSlashRequest = "/t/ms/basmeih/page/7/";
    expect(trailingSlashRequest).not.toBe(canonical);
    expect(trailingSlashRequest.endsWith("/")).toBe(true);
  });

  it("juzPathFor emits a slash-less canonical that a trailing-slash request pathname would diverge from", () => {
    const canonical = juzPathFor(ctx, 30);
    expect(canonical).toBe("/t/ms/basmeih/juz/30");
    expect(canonical.endsWith("/")).toBe(false);

    const trailingSlashRequest = "/t/ms/basmeih/juz/30/";
    expect(trailingSlashRequest).not.toBe(canonical);
    expect(trailingSlashRequest.endsWith("/")).toBe(true);
  });

  it("helper-built canonicals are deterministic across calls (same n -> same slash-less path)", () => {
    expect(globalPagePathFor(ctx, 7)).toBe(globalPagePathFor(ctx, 7));
    expect(juzPathFor(ctx, 30)).toBe(juzPathFor(ctx, 30));
    expect(hizbPathFor(ctx, 7)).toBe(hizbPathFor(ctx, 7));
    expect(rubPathFor(ctx, 7)).toBe(rubPathFor(ctx, 7));
  });

  it("helper-built canonical matches the sitemap route form for all range classes", () => {
    expect(globalPagePathFor(ctx, 7)).toBe(translationGlobalPagePath("ms", "basmeih", 7));
    expect(juzPathFor(ctx, 30)).toBe(translationJuzPath("ms", "basmeih", 30));
    for (const p of [globalPagePathFor(ctx, 7), juzPathFor(ctx, 30)]) {
      expect(p.endsWith("/")).toBe(false);
      expect(p.includes("/t/ms/basmeih/")).toBe(true);
    }
  });

  it("the translated range canonical is not the arabic range path", () => {
    expect(globalPagePathFor(ctx, 7)).not.toBe(globalPagePathFor(ARABIC, 7));
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
    expect(globalPagePathFor(ARABIC, 7)).toBe("/page/7");
    expect(juzPathFor(ARABIC, 30)).toBe("/juz/30");
  });
});
