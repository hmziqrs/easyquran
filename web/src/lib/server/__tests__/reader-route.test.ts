import { describe, expect, it } from "vite-plus/test";
import { reroute } from "../../../hooks";
import { parseReaderPath, parseReaderRoute } from "$lib/server/reader-route";

describe("localized reader semantic route parser", () => {
  it("accepts every canonical Arabic reader shape using Quran-data bounds", () => {
    expect(parseReaderPath("/app")).toEqual({ type: "index", page: "home" });
    expect(parseReaderPath("/app/juz")).toEqual({ type: "index", page: "juz" });
    expect(parseReaderPath("/app/al-fatihah")).toMatchObject({
      type: "arabic",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderPath("/app/al-baqarah/page/2")).toMatchObject({
      type: "arabic",
      cacheKind: "surah",
      index: 2,
      localPage: 2,
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
  });

  it("rejects unknown sources, noncanonical page one, malformed segments, and bad bounds", () => {
    expect(parseReaderPath("/app/al-fatihah/t/en/not-in-catalogue")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/page/1")).toBeNull();
    expect(parseReaderPath("/app/al-fatihah/t/en/sahih/page/1")).toBeNull();
    expect(parseReaderPath("/app/page/605")).toBeNull();
    expect(parseReaderPath("/app/juz/31")).toBeNull();
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
    expect(parseReaderRoute("/(marketing)", {})).toBeNull();
  });

  it("reroutes only supported locale and syntactically valid reader tuples", async () => {
    const route = async (pathname: string): Promise<string | void> =>
      reroute({ url: new URL(pathname, "https://easyquran.fyi"), fetch });

    expect(await route("/en/app/al-fatihah")).toBe("/app/al-fatihah");
    expect(await route("/ar/app/t/en/sahih/page/42")).toBe("/app/t/en/sahih/page/42");
    expect(await route("/de/app/al-fatihah")).toBe("/de/app/al-fatihah");
    expect(await route("/ar/account")).toBe("/ar/account");
    expect(await route("/en/app/page/0")).toBe("/en/app/page/0");
    expect(await route("/en/app/al-fatihah/t/en/sahih..int")).toBe(
      "/en/app/al-fatihah/t/en/sahih..int",
    );
    expect(await route("/en/app/al-fatihah%2Fpage%2F2")).toBe("/en/app/al-fatihah%2Fpage%2F2");
  });
});
