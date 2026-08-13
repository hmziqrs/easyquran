import {
  globalPagePathFor,
  juzPathFor,
  surahAyahPathFor,
  surahLocalPagePathFor,
  surahPathFor,
  surahRouteContext,
} from "$lib/data/quran";
import type { UiLocale } from "$lib/i18n/locales";
import {
  readerHomeHrefFor,
  readerHrefFor,
  type LocalizedReaderHref,
  type QuranReaderHref,
} from "$lib/i18n/reader";
import { describe, expect, expectTypeOf, it } from "vite-plus/test";

const VALID_READER_HREFS = [
  "/app",
  "/app/al-fatihah",
  "/app/al-baqarah/page/2",
  "/app/page/604",
  "/app/juz",
  "/app/juz/30",
  "/app/al-fatihah/t/en/sahih",
  "/app/al-baqarah/t/ms/basmeih/page/12",
  "/app/t/en/sahih/page/604",
  "/app/t/ru/kuliev-alsaadi/juz/30",
  "/app/al-fatihah/t/en/sahih.int",
] as const satisfies readonly QuranReaderHref[];

describe("reader localized hrefs", () => {
  it("prefixes every reader route family for both UI locales", () => {
    for (const locale of ["en", "ar"] as const) {
      for (const href of VALID_READER_HREFS) {
        expect(readerHrefFor(locale, href)).toBe(`/${locale}${href}`);
      }
    }
  });

  it("builds typed reader homes", () => {
    const english = readerHomeHrefFor("en");
    const arabic = readerHomeHrefFor("ar");
    expect(english).toBe("/en/app");
    expect(arabic).toBe("/ar/app");
    expectTypeOf(english).toEqualTypeOf<"/en/app">();
    expectTypeOf(arabic).toEqualTypeOf<"/ar/app">();
    expectTypeOf(readerHrefFor("ar", "/app/al-fatihah")).toEqualTypeOf<LocalizedReaderHref<"ar">>();
  });

  it("preserves translation segments, query, and fragment byte-for-byte", () => {
    expect(
      readerHrefFor(
        "ar",
        "/app/al-baqarah/t/ms/basmeih/page/8?view=focus&source=en.sahih#ayah-2-255",
      ),
    ).toBe("/ar/app/al-baqarah/t/ms/basmeih/page/8?view=focus&source=en.sahih#ayah-2-255");
    expect(readerHrefFor("en", "/app/al-fatihah?x=%2Fapp%2Ft#ayah-1-7")).toBe(
      "/en/app/al-fatihah?x=%2Fapp%2Ft#ayah-1-7",
    );
    expect(readerHrefFor("ar", "/app/al-fatihah?next=/../page/1&label=two%20words")).toBe(
      "/ar/app/al-fatihah?next=/../page/1&label=two%20words",
    );
    expect(readerHrefFor("ar", "/app/al-fatihah#ayah-1-1?kept=inside-fragment")).toBe(
      "/ar/app/al-fatihah#ayah-1-1?kept=inside-fragment",
    );
  });

  it("wraps every source-aware Quran helper without changing its source context", () => {
    const context = surahRouteContext("ms.basmeih");
    const surah = { slug: "ar-rum", num: 30 };
    const quranHrefs = [
      surahPathFor(context, surah),
      surahLocalPagePathFor(context, surah, 7),
      surahAyahPathFor(context, surah, 7, 12),
      globalPagePathFor(context, 42),
      juzPathFor(context, 30),
    ];

    for (const quranHref of quranHrefs) {
      const localized = readerHrefFor("ar", quranHref);
      expect(localized).toBe(`/ar${quranHref}`);
      expect(localized).toContain("/t/ms/basmeih");
      expect(localized).not.toContain("/t/ar/");
    }
  });

  it("rejects unsupported UI locales", () => {
    expect(() => readerHomeHrefFor("de" as UiLocale)).toThrowError(
      new TypeError("Unsupported UI locale: de"),
    );
    expect(() => readerHrefFor("de" as UiLocale, "/app/al-fatihah")).toThrowError(
      new TypeError("Unsupported UI locale: de"),
    );
  });
});

describe("reader href validation", () => {
  it.each([
    "https://easyquran.fyi/app/al-fatihah",
    "http://evil.test/app/al-fatihah",
    "javascript:/app/al-fatihah",
    "data:text/plain,/app/al-fatihah",
    "mailto:/app/al-fatihah",
    "//evil.test/app/al-fatihah",
    "///app/al-fatihah",
    "app/al-fatihah",
    "/",
    "/about",
    "/api/quran",
    "/en/app/al-fatihah",
    "/ar/app/al-fatihah",
  ])("rejects absolute, protocol-relative, localized, and non-reader input: %s", (href) => {
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "",
    "/app/",
    "/app//al-fatihah",
    "/app/al-fatihah/",
    "/app/al-fatihah//page/2",
    "/app/./al-fatihah",
    "/app/../app/al-fatihah",
    "/app/%2e/al-fatihah",
    "/app/%2E%2E/al-fatihah",
    "/app/al%2Ffatihah",
    "/app/al%5Cfatihah",
    "/app/al\\fatihah",
    "/app/al fatihah",
    "/app/al\tfatihah",
    "/app/al\nfatihah",
    "/app/الفاتحة",
    "/app/Al-Fatihah",
    "/app/-al-fatihah",
    "/app/al-fatihah-",
    "/app/al--fatihah",
  ])("rejects ambiguous or non-canonical pathname: %s", (href) => {
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/app/page",
    "/app/t",
    "/app/t/en/sahih",
    "/app/al-fatihah/t",
    "/app/al-fatihah/t/en",
    "/app/al-fatihah/t/en/sahih/juz/1",
    "/app/al-fatihah/foo/2",
    "/app/t/en/sahih/foo/2",
    "/app/t/en/sahih/page/2/extra",
    "/app/al-fatihah/t/en/sahih/page/2/extra",
  ])("rejects shapes outside reader grammar: %s", (href) => {
    expect(() => readerHrefFor("ar", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/app/page/0",
    "/app/page/01",
    "/app/page/-1",
    "/app/page/1.5",
    "/app/page/NaN",
    "/app/juz/Infinity",
    "/app/al-fatihah/page/1",
    "/app/al-fatihah/page/0",
    "/app/al-fatihah/t/en/sahih/page/1",
    "/app/al-fatihah/t/en/sahih/page/00",
    "/app/t/en/sahih/juz/+1",
  ])("rejects non-canonical numeric segments: %s", (href) => {
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/app/al-fatihah/t//sahih",
    "/app/al-fatihah/t/en/",
    "/app/al-fatihah/t/en/.sahih",
    "/app/al-fatihah/t/en/sahih.",
    "/app/al-fatihah/t/en/sahih..int",
    "/app/al-fatihah/t/en/-sahih",
    "/app/al-fatihah/t/en/sahih-",
    "/app/al-fatihah/t/en%2Dus/sahih",
    "/app/al-fatihah/t/en/sahih%2Eint",
  ])("rejects malformed or encoded source segments: %s", (href) => {
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/app/al-fatihah?bad=%",
    "/app/al-fatihah?bad=%2",
    "/app/al-fatihah?bad=%GG",
    "/app/al-fatihah?bad=%00",
    "/app/al-fatihah?bad=%09",
    "/app/al-fatihah?bad=%0A",
    "/app/al-fatihah?bad=%1F",
    "/app/al-fatihah?bad=%7f",
    "/app/al-fatihah#bad=%0",
    "/app/al-fatihah#one#two",
    "/app/al-fatihah?x=1\n#ayah-1-1",
    "/app/al-fatihah?x=raw space",
    "/app/al-fatihah?x=back\\slash",
    "/app/al-fatihah?",
    "/app/al-fatihah#",
    "/app/al-fatihah?#",
    "/app/al-fatihah?x=1#",
  ])("rejects malformed query or fragment: %s", (href) => {
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it("keeps semantic bounds outside prefix composition", () => {
    expect(readerHrefFor("en", "/app/page/605")).toBe("/en/app/page/605");
    expect(readerHrefFor("ar", "/app/juz/31")).toBe("/ar/app/juz/31");
  });

  it("rejects non-string href values at runtime", () => {
    expect(() => readerHrefFor("en", null as never)).toThrow(TypeError);
    expect(() => readerHrefFor("en", 42 as never)).toThrow(TypeError);
    expect(() => readerHrefFor("en", new String("/app") as never)).toThrow(TypeError);
  });
});
