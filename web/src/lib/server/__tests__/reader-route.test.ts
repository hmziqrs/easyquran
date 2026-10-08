import { describe, expect, it } from "vite-plus/test";

import { QURAN_DATA } from "#lib/server/quran-data.js";
import {
  parseReaderPath,
  parseReaderRoute,
  surahLocalRedirectTarget,
} from "#lib/server/reader-route.js";

import { reroute } from "../../../hooks";

describe("prefix-less reader semantic route parser", () => {
  it("accepts every canonical Arabic reader shape using Quran-data bounds", () => {
    expect(parseReaderPath("/juz")).toEqual({ type: "index", page: "juz" });
    expect(parseReaderPath("/surah")).toEqual({ type: "index", page: "surah" });
    expect(parseReaderPath("/pages")).toEqual({ type: "index", page: "pages" });
    expect(parseReaderPath("/al-fatihah")).toMatchObject({
      type: "arabic",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderPath("/page/604")).toMatchObject({
      type: "arabic",
      cacheKind: "page",
      index: 604,
    });
    expect(parseReaderPath("/juz/30")).toMatchObject({
      type: "arabic",
      cacheKind: "juz",
      index: 30,
    });
    expect(parseReaderPath("/hizb/60")).toMatchObject({
      type: "arabic",
      cacheKind: "hizb",
      index: 60,
    });
    expect(parseReaderPath("/rub/240")).toMatchObject({
      type: "arabic",
      cacheKind: "rub",
      index: 240,
    });
  });

  it("accepts baked translations but never derives a source from UI locale", () => {
    expect(parseReaderPath("/al-fatihah/t/en/sahih")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      contentLanguage: "en",
      contentDirection: "ltr",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderPath("/t/en/sahih/page/604")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "page",
      index: 604,
    });
    expect(parseReaderPath("/t/en/sahih/juz/30")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "juz",
      index: 30,
    });
    expect(parseReaderPath("/t/en/sahih/hizb/1")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "hizb",
      index: 1,
    });
    expect(parseReaderPath("/t/en/sahih/rub/240")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "rub",
      index: 240,
    });
  });

  it("rejects the deleted hub, unknown sources, removed surah-local shapes, malformed segments, and bad bounds", () => {
    // The /app hub is gone; neither spelling parses as a reader route.
    expect(parseReaderPath("/")).toBeNull();
    expect(parseReaderPath("/app")).toBeNull();
    expect(parseReaderPath("/al-fatihah/t/en/not-in-catalogue")).toBeNull();
    // The surah-local page scheme is removed; every /page/ shape under a slug is gone.
    expect(parseReaderPath("/al-baqarah/page/2")).toBeNull();
    expect(parseReaderPath("/al-fatihah/page/1")).toBeNull();
    expect(parseReaderPath("/al-fatihah/t/en/sahih/page/1")).toBeNull();
    expect(parseReaderPath("/al-fatihah/t/en/sahih/page/2")).toBeNull();
    expect(parseReaderPath("/page/605")).toBeNull();
    expect(parseReaderPath("/juz/31")).toBeNull();
    expect(parseReaderPath("/hizb/61")).toBeNull();
    expect(parseReaderPath("/rub/241")).toBeNull();
    expect(parseReaderPath("/hizb/0")).toBeNull();
    expect(parseReaderPath("/al-fatihah/page/0")).toBeNull();
    expect(parseReaderPath("//al-fatihah")).toBeNull();
    expect(parseReaderPath("/al-fatihah%2fpage%2f2")).toBeNull();
    // Numeric paths are redirect-only (D14): never a parsed reader route.
    expect(parseReaderPath("/2")).toBeNull();
  });

  it("validates resolved route IDs and parameters through the same parser", () => {
    expect(
      parseReaderRoute("/(application)/[surah]/t/[lang]/[translator]", {
        surah: "al-fatihah",
        lang: "en",
        translator: "sahih",
      }),
    ).toMatchObject({ type: "translation", sourceId: "en.sahih", index: 1 });
    expect(
      parseReaderRoute("/(application)/[surah=surahSlug]", { surah: "al-fatihah" }),
    ).toMatchObject({ type: "arabic", cacheKind: "surah", index: 1 });
    expect(
      parseReaderRoute("/(application)/[surah]/t/[lang]/[translator]", {
        surah: "missing",
        lang: "en",
        translator: "sahih",
      }),
    ).toBeNull();
    expect(parseReaderRoute("/(application)/hizb/[n]", { n: "60" })).toMatchObject({
      type: "arabic",
      cacheKind: "hizb",
      index: 60,
    });
    expect(
      parseReaderRoute("/(application)/t/[lang]/[translator]/rub/[n]", {
        lang: "en",
        translator: "sahih",
        n: "240",
      }),
    ).toMatchObject({ type: "translation", cacheKind: "rub", index: 240 });
    // .md twins resolve through the same parser after the twin suffix is dropped.
    expect(
      parseReaderRoute("/(application)/[surah=surahSlug].md", { surah: "al-fatihah" }),
    ).toMatchObject({ type: "arabic", cacheKind: "surah", index: 1 });
    expect(parseReaderRoute("/(marketing)", {})).toBeNull();
    // Bounded product pages are live localized routes, not reader routes.
    expect(parseReaderRoute("/(application)/settings", {})).toBeNull();
    expect(parseReaderRoute("/(application)/search", {})).toBeNull();
    expect(parseReaderRoute("/(application)/bookmarks", {})).toBeNull();
    expect(parseReaderRoute("/(application)/yours", {})).toBeNull();
  });
});

describe("reroute table (scheme A)", () => {
  const route = async (pathname: string): Promise<string | void> =>
    reroute({ url: new URL(pathname, "https://easyquran.fyi"), fetch });

  it("reroutes /ar/** onto the unprefixed tree for reader and product routes", async () => {
    expect(await route("/ar/al-fatihah")).toBe("/al-fatihah");
    expect(await route("/ar/t/en/sahih/page/42")).toBe("/t/en/sahih/page/42");
    expect(await route("/ar/juz")).toBe("/juz");
    expect(await route("/ar/rub/12.md")).toBe("/rub/12.md");
    expect(await route("/ar/search")).toBe("/search");
    expect(await route("/ar/settings")).toBe("/settings");
    expect(await route("/ar/bookmarks")).toBe("/bookmarks");
    expect(await route("/ar/yours")).toBe("/yours");
  });

  it("never reroutes /en/** (hooks.server owns the 308) or unknown locales", async () => {
    expect(await route("/en/app/al-fatihah")).toBe("/en/app/al-fatihah");
    expect(await route("/en/al-fatihah")).toBe("/en/al-fatihah");
    expect(await route("/de/al-fatihah")).toBe("/de/al-fatihah");
    expect(await route("/en/app/page/0")).toBe("/en/app/page/0");
    // Removed shapes no longer reroute into the app route tree; hooks 308 them.
    expect(await route("/app/al-fatihah/page/2")).toBe("/app/al-fatihah/page/2");
    expect(await route("/al-fatihah%2Fpage%2F2")).toBe("/al-fatihah%2Fpage%2F2");
  });

  it("keeps the /ar marketing-home remap", async () => {
    expect(await route("/ar")).toBe("/");
    expect(await route("/ar/")).toBe("/");
  });

  it("leaves unprefixed reader and marketing paths untouched", async () => {
    expect(await route("/al-fatihah")).toBe("/al-fatihah");
    expect(await route("/surah")).toBe("/surah");
    expect(await route("/search")).toBe("/search");
    expect(await route("/about")).toBe("/about");
  });
});

describe("surahLocalRedirectTarget (removed surah-local shapes 308 with an ayah anchor)", () => {
  it("maps an Arabic local page to the surah root anchored at the spread's first ayah", () => {
    const target = surahLocalRedirectTarget("/al-baqarah/page/2");
    const spread = QURAN_DATA.surahLocalPage(2, 2);
    if (!spread) throw new Error("missing al-baqarah local page 2");
    expect(target).toEqual({ path: "/al-baqarah", fragment: `#ayah-2-${spread.startAyah}` });
  });

  it("collapses local page 1 to the bare root with no fragment", () => {
    expect(surahLocalRedirectTarget("/al-fatihah/page/1")).toEqual({
      path: "/al-fatihah",
      fragment: "",
    });
  });

  it("keeps the translation segments on the translated shape", () => {
    const target = surahLocalRedirectTarget("/ar-rum/t/ms/basmeih/page/7");
    expect(target).not.toBeNull();
    expect(target?.path).toBe("/ar-rum/t/ms/basmeih");
    expect(target?.fragment).toMatch(/^#ayah-30-\d+$/);
  });

  it("yields null for unknown slugs, unknown translations, and out-of-range pages", () => {
    expect(surahLocalRedirectTarget("/not-a-surah/page/2")).toBeNull();
    expect(surahLocalRedirectTarget("/al-fatihah/t/en/not-in-catalogue/page/2")).toBeNull();
    expect(surahLocalRedirectTarget("/al-fatihah/page/99")).toBeNull();
    expect(surahLocalRedirectTarget("/al-fatihah")).toBeNull();
    expect(surahLocalRedirectTarget("/page/2")).toBeNull();
    // Legacy /app spellings belong to the hooks 308 map, never this parser.
    expect(surahLocalRedirectTarget("/app/al-baqarah/page/2")).toBeNull();
  });
});
