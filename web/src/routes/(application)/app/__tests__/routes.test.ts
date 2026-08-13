import {
  globalPagePathFor,
  juzPathFor,
  resumeCtxFor,
  surahAyahPath,
  surahAyahPathFor,
  surahLocalPagePath,
  surahLocalPagePathFor,
  surahPath,
  surahPathFor,
  surahRouteContext,
  translationGlobalPagePath,
  translationIdFromSegments,
  translationJuzPath,
  translationSegmentsFromId,
  translationSurahPath,
} from "$lib/data/quran";
import { describe, expect, it } from "vite-plus/test";

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

  it("cross-surah Next on a translated surah's last local page keeps t/<lang>/<translator> (ar-rum -> luqman)", () => {
    const next = surahLocalPagePathFor(ctx, nextSurah, 1);
    expect(next).toBe(`/app/${nextSurah}/t/ms/basmeih`);
    expect(next.endsWith("/page/1")).toBe(false);
    expect(surahPathFor(ctx, nextSurah)).toBe(next);
  });

  it("cross-surah Previous on local page 1 of a translated surah keeps t/<lang>/<translator> (ar-rum -> al-ankabut)", () => {
    const prev = surahLocalPagePathFor(ctx, prevSurah, 1);
    expect(prev).toBe(`/app/${prevSurah}/t/ms/basmeih`);
    expect(prev.endsWith("/page/1")).toBe(false);
    expect(surahPathFor(ctx, prevSurah)).toBe(prev);
  });
});

describe("route-aware path builders preserve translation within a surah", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const surah = "ar-rum";

  it("within-surah Next keeps translation after a forward-load (lastLoadedLocalPage !== initial.page.localPage)", () => {
    const next = surahLocalPagePathFor(ctx, surah, 8);
    expect(next).toBe(`/app/${surah}/t/ms/basmeih/page/8`);
    expect(next).not.toBe(surahLocalPagePath(surah, 8));
  });

  it("within-surah Previous keeps translation after a forward-load", () => {
    const prev = surahLocalPagePathFor(ctx, surah, 6);
    expect(prev).toBe(`/app/${surah}/t/ms/basmeih/page/6`);
    expect(prev).not.toBe(surahLocalPagePath(surah, 6));
  });

  it("collapses translation local page 1 to the bare translation route", () => {
    expect(surahLocalPagePathFor(ctx, surah, 1)).toBe(`/app/${surah}/t/ms/basmeih`);
    expect(surahLocalPagePathFor(ctx, surah, 1).endsWith("/page/1")).toBe(false);
  });
});

describe("route-aware ayah path preserves translation (reveal, sidebar, search, continue reading)", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const surah = { slug: "ar-rum", num: 30 };

  it("surahAyahPathFor keeps the translation segment and matches the translated local page", () => {
    const same = surahAyahPathFor(ctx, surah, 7, 12);
    expect(same).toBe("/app/ar-rum/t/ms/basmeih/page/7#ayah-30-12");
    expect(same.includes("/t/ms/basmeih/")).toBe(true);

    const cross = surahAyahPathFor(ctx, { slug: "luqman", num: 31 }, 1, 4);
    expect(cross).toBe("/app/luqman/t/ms/basmeih#ayah-31-4");
    expect(cross.endsWith("/page/1")).toBe(false);
  });

  it("surahAyahPathFor diverges from the Arabic-only surahAyahPath on a translation route", () => {
    expect(surahAyahPathFor(ctx, surah, 7, 12)).not.toBe(surahAyahPath(surah, 7, 12));
    expect(surahAyahPath(surah, 7, 12)).toBe("/app/ar-rum/page/7#ayah-30-12");
  });
});

describe("canonical/surah-page path for a translation source is not the Arabic path", () => {
  const ctx = surahRouteContext("ms.basmeih");

  it("surahLocalPagePathFor emits the translated canonical, not the Arabic one", () => {
    const canonical = surahLocalPagePathFor(ctx, "ar-rum", 7);
    expect(canonical).toBe("/app/ar-rum/t/ms/basmeih/page/7");
    expect(canonical).not.toBe(surahLocalPagePath("ar-rum", 7));
    expect(canonical.includes("/t/ms/basmeih/")).toBe(true);
  });
});

describe("arabic source context stays parity with legacy helpers", () => {
  const ctx = surahRouteContext("uthmani");
  const surah = { slug: "al-baqarah", num: 2 };

  it("surahPathFor / surahLocalPagePathFor / surahAyahPathFor match the Arabic-only builders", () => {
    expect(surahPathFor(ctx, "al-baqarah")).toBe(surahPath("al-baqarah"));
    expect(surahLocalPagePathFor(ctx, "al-baqarah", 3)).toBe(surahLocalPagePath("al-baqarah", 3));
    expect(surahLocalPagePathFor(ctx, "al-baqarah", 1)).toBe(surahLocalPagePath("al-baqarah", 1));
    expect(surahLocalPagePathFor(ctx, "al-baqarah", 1).endsWith("/page/1")).toBe(false);
    expect(surahAyahPathFor(ctx, surah, 3, 5)).toBe(surahAyahPath(surah, 3, 5));
  });

  it("never introduces a /t/ segment for an arabic source", () => {
    expect(surahPathFor(ctx, "al-baqarah").includes("/t/")).toBe(false);
    expect(surahLocalPagePathFor(ctx, "al-baqarah", 3).includes("/t/")).toBe(false);
    expect(surahAyahPathFor(ctx, surah, 3, 5).includes("/t/")).toBe(false);
  });
});

describe("legacy Arabic-only helpers drop the translation segment (regression guard)", () => {
  it("surahLocalPagePath never emits a /t/ segment", () => {
    expect(surahLocalPagePath("luqman", 1)).toBe("/app/luqman");
    expect(surahLocalPagePath("luqman", 1).includes("/t/")).toBe(false);
    expect(surahLocalPagePath("ar-rum", 8).includes("/t/")).toBe(false);
  });

  it("surahAyahPath never emits a /t/ segment", () => {
    expect(surahAyahPath({ slug: "ar-rum", num: 30 }, 7, 12)).toBe("/app/ar-rum/page/7#ayah-30-12");
    expect(surahAyahPath({ slug: "ar-rum", num: 30 }, 7, 12).includes("/t/")).toBe(false);
  });

  it("surahPath never emits a /t/ segment", () => {
    expect(surahPath("luqman").includes("/t/")).toBe(false);
  });
});

describe("sitemap emits every indexable route class (canonical-seo guard)", () => {
  const ARABIC = surahRouteContext("uthmani");
  const ctx = surahRouteContext("ms.basmeih");
  const slug = "ar-rum";

  it("emits the arabic global page route /app/page/[n] via globalPagePathFor", () => {
    expect(globalPagePathFor(ARABIC, 42)).toBe("/app/page/42");
    expect(globalPagePathFor(ARABIC, 1)).toBe("/app/page/1");
  });

  it("emits the translated global page route /app/t/<lang>/<translator>/page/[n] via globalPagePathFor", () => {
    expect(globalPagePathFor(ctx, 42)).toBe("/app/t/ms/basmeih/page/42");
    expect(globalPagePathFor(ctx, 42).includes("/t/ms/basmeih/")).toBe(true);
  });

  it("emits the translated surah local page route /app/<slug>/t/<lang>/<translator>/page/[localPage]", () => {
    expect(surahLocalPagePathFor(ctx, slug, 7)).toBe("/app/ar-rum/t/ms/basmeih/page/7");
    expect(surahLocalPagePathFor(ctx, slug, 7).includes("/t/ms/basmeih/")).toBe(true);
  });

  it("translation route classes are not silently collapsed to their arabic counterparts", () => {
    expect(globalPagePathFor(ctx, 42)).not.toBe(globalPagePathFor(ARABIC, 42));
    expect(surahLocalPagePathFor(ctx, slug, 7)).not.toBe(surahLocalPagePathFor(ARABIC, slug, 7));
  });

  it("emits both arabic and translation juz routes, kept distinct", () => {
    expect(juzPathFor(ARABIC, 30)).toBe("/app/juz/30");
    expect(juzPathFor(ctx, 30)).toBe("/app/t/ms/basmeih/juz/30");
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
  });
});

describe("/app entry redirect honors the persisted translation preference (secondary-surface guard)", () => {
  const surah = { slug: "al-baqarah", num: 2 };

  it("surahPathFor(surahRouteContext(sourceId), surah) keeps the persisted translation segment", () => {
    const ctx = surahRouteContext("ms.basmeih");
    expect(surahPathFor(ctx, surah)).toBe("/app/al-baqarah/t/ms/basmeih");
    expect(surahPathFor(ctx, surah).includes("/t/ms/basmeih")).toBe(true);
  });

  it("the redirect path diverges from the buggy arabic-only surahPath(surah)", () => {
    const ctx = surahRouteContext("ms.basmeih");
    expect(surahPath(surah)).toBe("/app/al-baqarah");
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
    const resume = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteArabic),
      surah,
      1,
      lastRead.n,
    );
    expect(resume).toBe("/app/luqman/t/ms/basmeih#ayah-31-4");
    expect(resume.includes("/t/ms/basmeih")).toBe(true);
  });

  it("resume into an arabic source drops /t/ even when the current route is a translation", () => {
    const lastRead: LastRead = { num: 30, n: 12, sourceId: "uthmani" };
    const surah = { slug: "ar-rum", num: 30 };
    const resume = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteTranslation),
      surah,
      7,
      lastRead.n,
    );
    expect(resume).toBe("/app/ar-rum/page/7#ayah-30-12");
    expect(resume.includes("/t/")).toBe(false);
  });

  it("the resume URL diverges from what the current routeContext alone would have produced", () => {
    const lastRead: LastRead = { num: 31, n: 4, sourceId: "ms.basmeih" };
    const surah = { slug: "luqman", num: 31 };
    const buggy = surahAyahPathFor(currentRouteArabic, surah, 1, lastRead.n);
    const fixed = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteArabic),
      surah,
      1,
      lastRead.n,
    );
    expect(fixed).not.toBe(buggy);
    expect(buggy.includes("/t/")).toBe(false);
    expect(fixed.includes("/t/ms/basmeih")).toBe(true);
  });

  it("cross-surah resume preserves translation across a surah boundary (ar-rum -> luqman, page 1 collapses)", () => {
    const lastRead: LastRead = { num: 31, n: 4, sourceId: "ms.basmeih" };
    const surah = { slug: "luqman", num: 31 };
    const resume = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteArabic),
      surah,
      1,
      lastRead.n,
    );
    expect(resume).toBe("/app/luqman/t/ms/basmeih#ayah-31-4");
    expect(resume.endsWith("/page/1")).toBe(false);
  });

  it("within-surah resume preserves translation on a deeper local page", () => {
    const lastRead: LastRead = { num: 30, n: 12, sourceId: "ms.basmeih" };
    const surah = { slug: "ar-rum", num: 30 };
    const resume = surahAyahPathFor(
      resumeCtxFor(lastRead, currentRouteArabic),
      surah,
      7,
      lastRead.n,
    );
    expect(resume).toBe("/app/ar-rum/t/ms/basmeih/page/7#ayah-30-12");
    expect(resume).not.toBe(surahAyahPath(surah, 7, lastRead.n));
  });

  it("falls back to the current routeContext when lastRead carries no sourceId (backwards compat)", () => {
    const lastRead: LastRead = { num: 30, n: 12 };
    const surah = { slug: "ar-rum", num: 30 };
    const ctx = resumeCtxFor(lastRead, currentRouteTranslation);
    expect(ctx).toBe(currentRouteTranslation);
    const resume = surahAyahPathFor(ctx, surah, 7, lastRead.n);
    expect(resume).toBe("/app/ar-rum/t/ms/basmeih/page/7#ayah-30-12");
  });
});

describe("translated range-route canonical is helper-built, not page.url.pathname (canonical-seo)", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const ARABIC = surahRouteContext("uthmani");

  it("globalPagePathFor emits a slash-less canonical that a trailing-slash request pathname would diverge from", () => {
    const canonical = globalPagePathFor(ctx, 7);
    expect(canonical).toBe("/app/t/ms/basmeih/page/7");
    expect(canonical.endsWith("/")).toBe(false);

    const trailingSlashRequest = "/app/t/ms/basmeih/page/7/";
    expect(trailingSlashRequest).not.toBe(canonical);
    expect(trailingSlashRequest.endsWith("/")).toBe(true);
  });

  it("juzPathFor emits a slash-less canonical that a trailing-slash request pathname would diverge from", () => {
    const canonical = juzPathFor(ctx, 30);
    expect(canonical).toBe("/app/t/ms/basmeih/juz/30");
    expect(canonical.endsWith("/")).toBe(false);

    const trailingSlashRequest = "/app/t/ms/basmeih/juz/30/";
    expect(trailingSlashRequest).not.toBe(canonical);
    expect(trailingSlashRequest.endsWith("/")).toBe(true);
  });

  it("helper-built canonicals are deterministic across calls (same n -> same slash-less path)", () => {
    expect(globalPagePathFor(ctx, 7)).toBe(globalPagePathFor(ctx, 7));
    expect(juzPathFor(ctx, 30)).toBe(juzPathFor(ctx, 30));
    expect(globalPagePathFor(ctx, 7).endsWith("/")).toBe(false);
    expect(juzPathFor(ctx, 30).endsWith("/")).toBe(false);
  });

  it("helper-built canonical matches the sitemap route form for both range classes", () => {
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
    expect(globalPagePathFor(ARABIC, 7)).toBe("/app/page/7");
    expect(juzPathFor(ARABIC, 30)).toBe("/app/juz/30");
  });
});
