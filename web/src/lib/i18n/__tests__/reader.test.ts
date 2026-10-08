import { describe, expect, expectTypeOf, it } from "vite-plus/test";

import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  rubPathFor,
  surahAyahPathFor,
  surahPathFor,
  surahRouteContext,
} from "#lib/data/quran.js";
import type { UiLocale } from "#lib/i18n/locales.js";
import {
  bookmarksPageHref,
  readerHrefFor,
  type LocalizedReaderHref,
  type QuranReaderHref,
} from "#lib/i18n/reader.js";

const VALID_READER_HREFS = [
  "/surah",
  "/al-fatihah",
  "/page/604",
  "/juz",
  "/juz/30",
  "/hizb/60",
  "/rub/240",
  "/al-fatihah/t/en/sahih",
  "/t/en/sahih/page/604",
  "/t/en/sahih/hizb/12",
  "/t/ru/kuliev-alsaadi/juz/30",
  "/al-fatihah/t/en/sahih.int",
] as const satisfies readonly QuranReaderHref[];

describe("reader localized hrefs", () => {
  it("keeps en unprefixed and prefixes ar on every reader route family", () => {
    for (const href of VALID_READER_HREFS) {
      expect(readerHrefFor("en", href)).toBe(href);
      expect(readerHrefFor("ar", href)).toBe(`/ar${href}`);
    }
  });

  it("builds the typed localized union for both locales", () => {
    expectTypeOf(readerHrefFor("en", "/al-fatihah")).toEqualTypeOf<QuranReaderHref>();
    expectTypeOf(readerHrefFor("ar", "/al-fatihah")).toEqualTypeOf<LocalizedReaderHref<"ar">>();
  });

  it("emits the canonical unlocalized /bookmarks nav href (settings/search precedent)", () => {
    const href = bookmarksPageHref();
    // /ar/bookmarks is not a published route — hooks.server 404s a
    // locale-prefixed variant on hard load, so nav/footer hrefs must stay bare.
    expect(href).toBe("/bookmarks");
    expect(href).not.toMatch(/^\/ar\//u);
    expectTypeOf(href).toEqualTypeOf<"/bookmarks">();
  });

  it("preserves translation segments, query, and fragment byte-for-byte", () => {
    expect(
      readerHrefFor("ar", "/al-baqarah/t/ms/basmeih?view=focus&source=en.sahih#ayah-2-255"),
    ).toBe("/ar/al-baqarah/t/ms/basmeih?view=focus&source=en.sahih#ayah-2-255");
    expect(readerHrefFor("en", "/al-fatihah?x=%2Ft%2Fen%2Fsahih#ayah-1-7")).toBe(
      "/al-fatihah?x=%2Ft%2Fen%2Fsahih#ayah-1-7",
    );
    expect(readerHrefFor("ar", "/al-fatihah?next=/../page/1&label=two%20words")).toBe(
      "/ar/al-fatihah?next=/../page/1&label=two%20words",
    );
    expect(readerHrefFor("ar", "/al-fatihah#ayah-1-1?kept=inside-fragment")).toBe(
      "/ar/al-fatihah#ayah-1-1?kept=inside-fragment",
    );
  });

  it("wraps every source-aware Quran helper without changing its source context", () => {
    const context = surahRouteContext("ms.basmeih");
    const surah = { slug: "ar-rum", num: 30 };
    const quranHrefs = [
      surahPathFor(context, surah),
      surahAyahPathFor(context, surah, 12),
      globalPagePathFor(context, 42),
      juzPathFor(context, 30),
      hizbPathFor(context, 60),
      rubPathFor(context, 240),
    ];

    for (const quranHref of quranHrefs) {
      const localized = readerHrefFor("ar", quranHref);
      expect(localized).toBe(`/ar${quranHref}`);
      expect(localized).toContain("/t/ms/basmeih");
      expect(localized).not.toContain("/t/ar/");
    }
  });

  it("rejects unsupported UI locales", () => {
    // SAFETY: "de" is deliberately not a UiLocale — the casts feed the unsupported-locale throw path.
    expect(() => readerHrefFor("de" as UiLocale, "/al-fatihah")).toThrowError(
      new TypeError("Unsupported UI locale: de"),
    );
  });
});

describe("reader href validation", () => {
  it.each([
    "https://easyquran.fyi/al-fatihah",
    "http://evil.test/al-fatihah",
    "javascript:/al-fatihah",
    "data:text/plain,/al-fatihah",
    "mailto:/al-fatihah",
    "//evil.test/al-fatihah",
    "///al-fatihah",
    "al-fatihah",
    "/",
    "/about",
    "/api/quran",
    // locale prefixes are never part of a canonical href (scheme A)
    "/en/al-fatihah",
    "/ar/al-fatihah",
    // the deleted /app spellings are legacy, not canonical
    "/app/al-fatihah",
    "/en/app/al-fatihah",
  ])("rejects absolute, protocol-relative, localized, and non-reader input: %s", (href) => {
    // SAFETY: each fixture is deliberately a non-canonical href; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "",
    "/al-fatihah/",
    "/al-fatihah//page/2",
    "/./al-fatihah",
    "/../al-fatihah",
    "/%2e/al-fatihah",
    "/%2E%2E/al-fatihah",
    "/al%2Ffatihah",
    "/al%5Cfatihah",
    "/al\\fatihah",
    "/al fatihah",
    "/al\tfatihah",
    "/al\nfatihah",
    "/الفاتحة",
    "/Al-Fatihah",
    "/-al-fatihah",
    "/al-fatihah-",
    "/al--fatihah",
    "/_reader/al-fatihah",
  ])("rejects ambiguous or non-canonical pathname: %s", (href) => {
    // SAFETY: each fixture is deliberately a non-canonical pathname; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/page",
    "/t",
    "/t/en/sahih",
    "/al-fatihah/t",
    "/al-fatihah/t/en",
    "/al-fatihah/t/en/sahih/juz/1",
    "/al-fatihah/foo/2",
    "/t/en/sahih/foo/2",
    "/t/en/sahih/page/2/extra",
    "/al-fatihah/t/en/sahih/page/2/extra",
  ])("rejects shapes outside reader grammar: %s", (href) => {
    // SAFETY: each fixture is deliberately outside the reader grammar; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("ar", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/page/0",
    "/page/01",
    "/page/-1",
    "/page/1.5",
    "/page/NaN",
    "/juz/Infinity",
    "/al-fatihah/page/1",
    "/al-fatihah/page/0",
    "/al-fatihah/t/en/sahih/page/1",
    "/al-fatihah/t/en/sahih/page/00",
    "/t/en/sahih/juz/+1",
  ])("rejects non-canonical numeric segments: %s", (href) => {
    // SAFETY: each fixture is deliberately a non-canonical numeric segment; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/al-fatihah/t//sahih",
    "/al-fatihah/t/en/",
    "/al-fatihah/t/en/.sahih",
    "/al-fatihah/t/en/sahih.",
    "/al-fatihah/t/en/sahih..int",
    "/al-fatihah/t/en/-sahih",
    "/al-fatihah/t/en/sahih-",
    "/al-fatihah/t/en%2Dus/sahih",
    "/al-fatihah/t/en/sahih%2Eint",
  ])("rejects malformed or encoded source segments: %s", (href) => {
    // SAFETY: each fixture is deliberately a malformed/encoded source segment; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it.each([
    "/al-fatihah?bad=%",
    "/al-fatihah?bad=%2",
    "/al-fatihah?bad=%GG",
    "/al-fatihah?bad=%00",
    "/al-fatihah?bad=%09",
    "/al-fatihah?bad=%0A",
    "/al-fatihah?bad=%1F",
    "/al-fatihah?bad=%7f",
    "/al-fatihah#bad=%0",
    "/al-fatihah#one#two",
    "/al-fatihah?x=1\n#ayah-1-1",
    "/al-fatihah?x=raw space",
    "/al-fatihah?x=back\\slash",
    "/al-fatihah?",
    "/al-fatihah#",
    "/al-fatihah?#",
    "/al-fatihah?x=1#",
  ])("rejects malformed query or fragment: %s", (href) => {
    // SAFETY: each fixture is deliberately a malformed query/fragment; the cast only silences the compiler so the rejection path runs.
    expect(() => readerHrefFor("en", href as QuranReaderHref)).toThrow(TypeError);
  });

  it("keeps semantic bounds outside prefix composition", () => {
    expect(readerHrefFor("en", "/page/605")).toBe("/page/605");
    expect(readerHrefFor("ar", "/juz/31")).toBe("/ar/juz/31");
  });

  it("rejects non-string href values at runtime", () => {
    // SAFETY: null is deliberately not a href — exercises the runtime typeof rejection in isCanonicalReaderHref.
    expect(() => readerHrefFor("en", null as never)).toThrow(TypeError);
    // SAFETY: 42 is deliberately not a href — exercises the runtime typeof rejection in isCanonicalReaderHref.
    expect(() => readerHrefFor("en", 42 as never)).toThrow(TypeError);
    // SAFETY: a String object is deliberately not a primitive href — exercises the runtime typeof rejection in isCanonicalReaderHref.
    expect(() => readerHrefFor("en", new String("/al-fatihah") as never)).toThrow(TypeError);
  });
});
