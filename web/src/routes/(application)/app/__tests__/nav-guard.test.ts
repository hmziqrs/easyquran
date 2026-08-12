import { describe, expect, it } from "vite-plus/test";
import {
  globalPagePathFor,
  juzPathFor,
  routeContextFromParams,
  surahAyahPathFor,
  surahLocalPagePathFor,
  surahPathFor,
  surahRouteContext,
  translationIdFromSegments,
} from "$lib/data/quran";

const ARABIC_ONLY_HELPERS = /\bsurahPath\b|\bsurahLocalPagePath\b|\bsurahAyahPath\b/;
const HAND_BUILT_APP_NAV = /\/app\/(?!\$\{string\})/;
const NAV_SIGNAL = /(?:\bhref\s*=|\bgoto\s*\(|\bresolve\s*\()/;

/**
 * Everything that can produce reader navigation: route components, shared
 * components, and the global-search palette sources. `$lib/server` is excluded
 * on purpose — prerendered Arabic-only output is the one legitimate caller of
 * the Arabic-only helpers. Glob options must be inline object literals.
 */
const components = {
  ...(import.meta.glob("../../../**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
  ...(import.meta.glob("../../../../lib/components/**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
  ...(import.meta.glob("../../../../lib/search/**/*.ts", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
};

function stripComments(src: string): string {
  return src
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/[^\n]*/g, "$1");
}

function handBuiltAppNavHits(src: string): string[] {
  const hits: string[] = [];
  for (const line of stripComments(src).split("\n")) {
    if (NAV_SIGNAL.test(line) && HAND_BUILT_APP_NAV.test(line)) hits.push(line.trim());
  }
  return hits;
}

describe("reader navigation regression guard", () => {
  it("the centralized route-aware helpers are exported from $lib/data/quran", () => {
    expect(typeof surahPathFor).toBe("function");
    expect(typeof surahLocalPagePathFor).toBe("function");
    expect(typeof surahAyahPathFor).toBe("function");
    expect(typeof globalPagePathFor).toBe("function");
    expect(typeof juzPathFor).toBe("function");
    expect(typeof surahRouteContext).toBe("function");
  });

  it("no component or palette source references the Arabic-only path helpers directly", () => {
    for (const [path, src] of Object.entries(components)) {
      expect(
        src,
        `Arabic-only helper used in ${path} -> use a *For(ctx, ...) helper instead`,
      ).not.toMatch(ARABIC_ONLY_HELPERS);
    }
  });

  it("the guard regex never matches the *For variants (false-positive check)", () => {
    expect(ARABIC_ONLY_HELPERS.test("surahPathFor")).toBe(false);
    expect(ARABIC_ONLY_HELPERS.test("surahLocalPagePathFor")).toBe(false);
    expect(ARABIC_ONLY_HELPERS.test("surahAyahPathFor")).toBe(false);
  });
});

describe("route components never hand-build /app/ navigation literals", () => {
  it("no component or palette source hand-builds a /app/ url used by href/goto/resolve", () => {
    for (const [path, src] of Object.entries(components)) {
      expect(
        handBuiltAppNavHits(src),
        `Hand-built /app/ navigation literal in ${path} -> use juzPathFor/globalPagePathFor/surah*For(ctx, ...) instead`,
      ).toEqual([]);
    }
  });

  it("the /app/ guard flags a hand-built range url", () => {
    expect(HAND_BUILT_APP_NAV.test("href={`/app/juz/${n}`}")).toBe(true);
    expect(HAND_BUILT_APP_NAV.test('goto("/app/page/3")')).toBe(true);
    expect(HAND_BUILT_APP_NAV.test("resolve(`/app/t/en/sahih/juz/30`)")).toBe(true);
  });

  it("the /app/ guard ignores the template-literal type placeholder", () => {
    expect(HAND_BUILT_APP_NAV.test("function f(): `/app/${string}` {")).toBe(false);
    expect(HAND_BUILT_APP_NAV.test("function g(x: `/app/${string}` | null): void {}")).toBe(false);
  });

  it("the nav-signal scope ignores non-navigation /app/ uses (seo path, comments)", () => {
    expect(handBuiltAppNavHits('<Seo path="/app/juz" />')).toEqual([]);
    expect(handBuiltAppNavHits("<Seo path={`/app/page/${n}`} />")).toEqual([]);
    expect(handBuiltAppNavHits('// href="/app/foo" was the old route')).toEqual([]);
    expect(handBuiltAppNavHits('/* <a href="/app/bar">legacy</a> */')).toEqual([]);
    expect(handBuiltAppNavHits('const url = "https://example.com/app/baz";')).toEqual([]);
  });
});

describe("translated range-route fixtures preserve active source context across navigation", () => {
  const ctx = surahRouteContext("ms.basmeih");
  const ARABIC = surahRouteContext("uthmani");

  it("routeContextFromParams keeps the translation segment for a translated range route", () => {
    expect(routeContextFromParams({ lang: "ms", translator: "basmeih", n: "30" })).toEqual({
      kind: "translation",
      lang: "ms",
      translator: "basmeih",
    });
    expect(routeContextFromParams({ n: "30" })).toEqual({ kind: "arabic" });
  });

  it("page navigation keeps /t/<lang>/<translator> via globalPagePathFor", () => {
    const prev = globalPagePathFor(ctx, 6);
    const next = globalPagePathFor(ctx, 8);
    expect(prev).toBe("/app/t/ms/basmeih/page/6");
    expect(next).toBe("/app/t/ms/basmeih/page/8");
    expect(prev.includes("/t/ms/basmeih/")).toBe(true);
    expect(next).not.toBe(globalPagePathFor(ARABIC, 8));
  });

  it("juz navigation keeps /t/<lang>/<translator> via juzPathFor (juz 30 on all paths)", () => {
    expect(juzPathFor(ctx, 30)).toBe("/app/t/ms/basmeih/juz/30");
    expect(juzPathFor(ARABIC, 30)).toBe("/app/juz/30");
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
    expect(juzPathFor(ctx, 30).includes("/t/ms/basmeih/")).toBe(true);
  });

  it("surah navigation from a range route keeps /t/<lang>/<translator> via surahPathFor", () => {
    const surahSlug = "ar-rum";
    expect(surahPathFor(ctx, surahSlug)).toBe("/app/ar-rum/t/ms/basmeih");
    expect(surahPathFor(ctx, surahSlug).includes("/t/ms/basmeih")).toBe(true);
    expect(surahPathFor(ctx, surahSlug)).not.toBe(surahPathFor(ARABIC, surahSlug));
  });

  it("ayah navigation from a range route keeps /t/<lang>/<translator> via surahAyahPathFor", () => {
    const surah = { slug: "ar-rum", num: 30 };
    const path = surahAyahPathFor(ctx, surah, 7, 12);
    expect(path).toBe("/app/ar-rum/t/ms/basmeih/page/7#ayah-30-12");
    expect(path.includes("/t/ms/basmeih/")).toBe(true);
    expect(path).not.toBe(surahAyahPathFor(ARABIC, surah, 7, 12));
  });

  it("translationIdFromSegments round-trips the active source id used by the reader", () => {
    if (ctx.kind !== "translation") throw new Error("expected translation ctx");
    const id = translationIdFromSegments(ctx.lang, ctx.translator);
    expect(id).toBe("ms.basmeih");
    expect(surahRouteContext(id)).toEqual(ctx);
  });
});
