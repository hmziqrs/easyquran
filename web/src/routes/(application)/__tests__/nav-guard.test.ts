import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  routeContextFromParams,
  rubPathFor,
  surahAyahPathFor,
  surahPathFor,
  surahRouteContext,
  translationIdFromSegments,
} from "$lib/data/quran";
import { describe, expect, it } from "vite-plus/test";

const ARABIC_ONLY_HELPERS = /\bsurahPath\b|\bsurahLocalPagePath\b|\bsurahAyahPath\b/;
const NAV_SIGNAL = /(?:\bhref\s*=|\bgoto\s*\(|\bresolve\s*\()/;

/**
 * Scheme A has no `/app` marker segment to lean on, so the guard runs two
 * regexes over NAV_SIGNAL-scoped, comment-stripped lines:
 *
 * (a) HAND_BUILT_APP_LITERAL — any literal `/app` path (the dead scheme's
 *     marker). `/application` and the template-literal type placeholder
 *     `` `/app/${string}` `` stay exempt; everything else on a navigation line
 *     is a stale hand-built URL.
 *
 * (b) HAND_BUILT_RESERVED_SEGMENT — a range/translation segment literal right
 *     after a string boundary with a path continuation (`href={"/page/" + n}`,
 *     `goto(`/t/en/sahih/juz/${n}`)`). Reader paths must come from the
 *     *For(ctx, …) builder family, whose outputs never open with these
 *     segments inside a navigation literal.
 */
const HAND_BUILT_APP_LITERAL = /\/app(?!lication)(?!\/\$\{string\})/u;
const HAND_BUILT_RESERVED_SEGMENT =
  /(?:^|["'`+\s(])\/(?:page|juz|hizb|rub|t)\/(?!page\/)(?![a-z0-9-]*\/t\/)/u;

/**
 * Everything that can produce reader navigation: route components, shared
 * components, and the global-search palette sources. `$lib/server` is excluded
 * on purpose — prerendered Arabic-only output is the one legitimate caller of
 * the Arabic-only helpers. Glob options must be inline object literals.
 */
const components = {
  // SAFETY: import.meta.glob with {query:'?raw', import:'default', eager:true} yields Record<string,string> of raw module sources
  ...(import.meta.glob("../**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
  // SAFETY: same glob invariant — raw eager default imports are string module sources
  ...(import.meta.glob("../../../lib/components/**/*.svelte", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>),
  // SAFETY: same glob invariant — raw eager default imports are string module sources
  ...(import.meta.glob("../../../lib/search/**/*.ts", {
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

function handBuiltNavHits(src: string): string[] {
  const hits: string[] = [];
  for (const line of stripComments(src).split("\n")) {
    if (!NAV_SIGNAL.test(line)) continue;
    if (HAND_BUILT_APP_LITERAL.test(line) || HAND_BUILT_RESERVED_SEGMENT.test(line)) {
      hits.push(line.trim());
    }
  }
  return hits;
}

describe("reader navigation regression guard", () => {
  it("the centralized route-aware helpers are exported from $lib/data/quran", () => {
    expect(surahPathFor).toBeInstanceOf(Function);
    expect(surahAyahPathFor).toBeInstanceOf(Function);
    expect(globalPagePathFor).toBeInstanceOf(Function);
    expect(juzPathFor).toBeInstanceOf(Function);
    expect(hizbPathFor).toBeInstanceOf(Function);
    expect(rubPathFor).toBeInstanceOf(Function);
    expect(surahRouteContext).toBeInstanceOf(Function);
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
    expect(ARABIC_ONLY_HELPERS.test("surahAyahPathFor")).toBe(false);
    // The removed surah-local scheme cannot sneak back in under a *For name.
    expect(ARABIC_ONLY_HELPERS.test("surahLocalPagePathFor")).toBe(false);
    expect(ARABIC_ONLY_HELPERS.test("const x = surahLocalPagePath")).toBe(true);
  });
});

describe("route components never hand-build navigation path literals", () => {
  it("no component or palette source hand-builds a reader url used by href/goto/resolve", () => {
    for (const [path, src] of Object.entries(components)) {
      expect(
        handBuiltNavHits(src),
        `Hand-built navigation literal in ${path} -> use juzPathFor/globalPagePathFor/surah*For(ctx, ...) instead`,
      ).toEqual([]);
    }
  });

  it("the literal /app guard flags the dead scheme's spellings", () => {
    expect(HAND_BUILT_APP_LITERAL.test('href="/app/surah"')).toBe(true);
    expect(HAND_BUILT_APP_LITERAL.test("goto(`/app/juz/${n}`)")).toBe(true);
    expect(HAND_BUILT_APP_LITERAL.test('href="/application-form"')).toBe(false);
  });

  it("the literal /app guard ignores the template-literal type placeholder", () => {
    expect(HAND_BUILT_APP_LITERAL.test("function f(): `/app/${string}` {")).toBe(false);
    expect(HAND_BUILT_APP_LITERAL.test("function g(x: `/app/${string}` | null): void {}")).toBe(
      false,
    );
  });

  it("the reserved-segment guard flags hand-built range/translation literals", () => {
    expect(HAND_BUILT_RESERVED_SEGMENT.test('href={"/page/" + n}')).toBe(true);
    expect(HAND_BUILT_RESERVED_SEGMENT.test("goto(`/juz/${n}`)")).toBe(true);
    expect(HAND_BUILT_RESERVED_SEGMENT.test("goto(`/t/en/sahih/juz/${n}`)")).toBe(true);
    expect(HAND_BUILT_RESERVED_SEGMENT.test('resolve("/hizb/44")')).toBe(true);
  });

  it("the reserved-segment guard ignores product pages and builder outputs", () => {
    expect(HAND_BUILT_RESERVED_SEGMENT.test('href="/settings"')).toBe(false);
    expect(HAND_BUILT_RESERVED_SEGMENT.test('href="/search"')).toBe(false);
    // Builder outputs embed /t/ mid-path after a slug segment — never after a
    // string boundary — and single /t-less indexes are literal-free.
    expect(HAND_BUILT_RESERVED_SEGMENT.test("href={publicHref(surahPathFor(ctx, slug))}")).toBe(
      false,
    );
  });

  it("the nav-signal scope ignores non-navigation uses (seo path, comments)", () => {
    expect(handBuiltNavHits('<Seo path="/juz" />')).toEqual([]);
    expect(handBuiltNavHits("<Seo path={`/page/${n}`} />")).toEqual([]);
    expect(handBuiltNavHits('// href="/foo" was the old route')).toEqual([]);
    expect(handBuiltNavHits('/* <a href="/bar">legacy</a> */')).toEqual([]);
    expect(handBuiltNavHits('const url = "https://example.com/baz";')).toEqual([]);
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
    expect(prev).toBe("/t/ms/basmeih/page/6");
    expect(next).toBe("/t/ms/basmeih/page/8");
    expect(prev.includes("/t/ms/basmeih/")).toBe(true);
    expect(next).not.toBe(globalPagePathFor(ARABIC, 8));
  });

  it("juz navigation keeps /t/<lang>/<translator> via juzPathFor (juz 30 on all paths)", () => {
    expect(juzPathFor(ctx, 30)).toBe("/t/ms/basmeih/juz/30");
    expect(juzPathFor(ARABIC, 30)).toBe("/juz/30");
    expect(juzPathFor(ctx, 30)).not.toBe(juzPathFor(ARABIC, 30));
    expect(juzPathFor(ctx, 30).includes("/t/ms/basmeih/")).toBe(true);
  });

  it("surah navigation from a range route keeps /t/<lang>/<translator> via surahPathFor", () => {
    const surahSlug = "ar-rum";
    expect(surahPathFor(ctx, surahSlug)).toBe("/ar-rum/t/ms/basmeih");
    expect(surahPathFor(ctx, surahSlug).includes("/t/ms/basmeih")).toBe(true);
    expect(surahPathFor(ctx, surahSlug)).not.toBe(surahPathFor(ARABIC, surahSlug));
  });

  it("ayah navigation from a range route keeps /t/<lang>/<translator> via surahAyahPathFor", () => {
    const surah = { slug: "ar-rum", num: 30 };
    const path = surahAyahPathFor(ctx, surah, 12);
    expect(path).toBe("/ar-rum/t/ms/basmeih#ayah-30-12");
    expect(path.includes("/t/ms/basmeih")).toBe(true);
    expect(path).not.toBe(surahAyahPathFor(ARABIC, surah, 12));
  });

  it("hizb and rub navigation keeps /t/<lang>/<translator> via hizbPathFor/rubPathFor", () => {
    expect(hizbPathFor(ctx, 60)).toBe("/t/ms/basmeih/hizb/60");
    expect(hizbPathFor(ARABIC, 1)).toBe("/hizb/1");
    expect(rubPathFor(ctx, 240)).toBe("/t/ms/basmeih/rub/240");
    expect(rubPathFor(ARABIC, 1)).toBe("/rub/1");
    expect(rubPathFor(ctx, 4)).not.toBe(rubPathFor(ARABIC, 4));
  });

  it("translationIdFromSegments round-trips the active source id used by the reader", () => {
    if (ctx.kind !== "translation") throw new Error("expected translation ctx");
    const id = translationIdFromSegments(ctx.lang, ctx.translator);
    expect(id).toBe("ms.basmeih");
    expect(surahRouteContext(id)).toEqual(ctx);
  });
});
