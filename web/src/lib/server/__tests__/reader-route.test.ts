import { QURAN_DATA } from "$lib/server/quran-data";
import {
  parseReaderPath,
  parseReaderRoute,
  surahLocalRedirectTarget,
} from "$lib/server/reader-route";
import { describe, expect, it } from "vite-plus/test";

import { reroute } from "../../../hooks";

describe("localized reader semantic route parser", () => {
  it("accepts every canonical Arabic reader shape using Quran-data bounds", () => {
    expect(parseReaderPath("/app")).toEqual({ type: "index", page: "home" });
    expect(parseReaderPath("/app/juz")).toEqual({ type: "index", page: "juz" });
    expect(parseReaderPath("/app/al-fatihah")).toMatchObject({
      type: "arabic",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderPath("/app/page/604")).toMatchObject({
      type: "arabic",
      cacheKind: "page",
      index: 604,
    });
    expect(parseReaderPath("/app/juz/30")).toMatchObject({
      type: "arabic",
      cacheKind: "juz",
      index: 30,
    });
    expect(parseReaderPath("/app/hizb/60")).toMatchObject({
      type: "arabic",
      cacheKind: "hizb",
      index: 60,
    });
    expect(parseReaderPath("/app/rub/240")).toMatchObject({
      type: "arabic",
      cacheKind: "rub",
      index: 240,
    });
  });

  it("accepts baked translations but never derives a source from UI locale", () => {
    expect(parseReaderPath("/app/al-fatihah/t/en/sahih")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      contentLanguage: "en",
      contentDirection: "ltr",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderPath("/app/t/en/sahih/page/604")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "page",
      index: 604,
    });
    expect(parseReaderPath("/app/t/en/sahih/juz/30")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "juz",
      index: 30,
    });
    expect(parseReaderPath("/app/t/en/sahih/hizb/1")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "hizb",
      index: 1,
    });
    expect(parseReaderPath("/app/t/en/sahih/rub/240")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih",
      cacheKind: "rub",
      index: 240,
    });
  });

  it("rejects unknown sources, removed surah-local shapes, malformed segments, and bad bounds", () => {
    expect(parseReaderPath("/app/al-fatihah/t/en/not-in-catalogue")).toBeNull();
    // The surah-local page scheme is removed; every /page/ shape under a slug is gone.
    expect(parseReaderPath("/app/al-baqarah/page/2")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/page/1")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/t/en/sahih/page/1")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/t/en/sahih/page/2")).toBeNull();
    expect(parseReaderPath("/app/page/605")).toBeNull();
    expect(parseReaderPath("/app/juz/31")).toBeNull();
    expect(parseReaderPath("/app/hizb/61")).toBeNull();
    expect(parseReaderPath("/app/rub/241")).toBeNull();
    expect(parseReaderPath("/app/hizb/0")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/page/0")).toBeNull();
    expect(parseReaderPath("/app//al-fatihah")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah%2fpage%2f2")).toBeNull();
  });

  it("validates resolved route IDs and parameters through the same parser", () => {
    expect(
      parseReaderRoute("/(application)/app/[surah]/t/[lang]/[translator]", {
        surah: "al-fatihah",
        lang: "en",
        translator: "sahih",
      }),
    ).toMatchObject({ type: "translation", sourceId: "en.sahih", index: 1 });
    expect(
      parseReaderRoute("/(application)/app/[surah]/t/[lang]/[translator]", {
        surah: "missing",
        lang: "en",
        translator: "sahih",
      }),
    ).toBeNull();
    expect(
      parseReaderRoute("/(application)/app/hizb/[n]", { n: "60" }),
    ).toMatchObject({ type: "arabic", cacheKind: "hizb", index: 60 });
    expect(
      parseReaderRoute("/(application)/app/t/[lang]/[translator]/rub/[n]", {
        lang: "en",
        translator: "sahih",
        n: "240",
      }),
    ).toMatchObject({ type: "translation", cacheKind: "rub", index: 240 });
    expect(parseReaderRoute("/(marketing)", {})).toBeNull();
  });

  it("reroutes only supported locale and syntactically valid reader tuples", async () => {
    const route = async (pathname: string): Promise<string | void> =>
      reroute({ url: new URL(pathname, "https://easyquran.fyi"), fetch });

    expect(await route("/en/app/al-fatihah")).toBe("/app/al-fatihah");
    expect(await route("/ar/app/t/en/sahih/page/42")).toBe("/app/t/en/sahih/page/42");
    expect(await route("/en/app/hizb/7")).toBe("/app/hizb/7");
    expect(await route("/en/app/rub/12.md")).toBe("/app/rub/12.md");
    expect(await route("/de/app/al-fatihah")).toBe("/de/app/al-fatihah");
    expect(await route("/ar/account")).toBe("/ar/account");
    expect(await route("/en/app/page/0")).toBe("/en/app/page/0");
    // Removed shapes no longer reroute into the app route tree; hooks 308 them.
    expect(await route("/en/app/al-fatihah/page/2")).toBe("/en/app/al-fatihah/page/2");
    expect(await route("/en/app/al-fatihah%2Fpage%2F2")).toBe("/en/app/al-fatihah%2Fpage%2F2");
  });
});

describe("surahLocalRedirectTarget (removed surah-local shapes 308 with an ayah anchor)", () => {
  it("maps an Arabic local page to the surah root anchored at the spread's first ayah", () => {
    const target = surahLocalRedirectTarget("/app/al-baqarah/page/2");
    const spread = QURAN_DATA.surahLocalPage(2, 2);
    if (!spread) throw new Error("missing al-baqarah local page 2");
    expect(target).toEqual({ path: "/app/al-baqarah", fragment: `#ayah-2-${spread.startAyah}` });
  });

  it("collapses local page 1 to the bare root with no fragment", () => {
    expect(surahLocalRedirectTarget("/app/al-fatihah/page/1")).toEqual({
      path: "/app/al-fatihah",
      fragment: "",
    });
  });

  it("keeps the translation segments on the translated shape", () => {
    const target = surahLocalRedirectTarget("/app/ar-rum/t/ms/basmeih/page/7");
    expect(target).not.toBeNull();
    expect(target?.path).toBe("/app/ar-rum/t/ms/basmeih");
    expect(target?.fragment).toMatch(/^#ayah-30-\d+$/);
  });

  it("yields null for unknown slugs, unknown translations, and out-of-range pages", () => {
    expect(surahLocalRedirectTarget("/app/not-a-surah/page/2")).toBeNull();
    expect(surahLocalRedirectTarget("/app/al-fatihah/t/en/not-in-catalogue/page/2")).toBeNull();
    expect(surahLocalRedirectTarget("/app/al-fatihah/page/99")).toBeNull();
    expect(surahLocalRedirectTarget("/app/al-fatihah")).toBeNull();
    expect(surahLocalRedirectTarget("/app/page/2")).toBeNull();
  });
});
