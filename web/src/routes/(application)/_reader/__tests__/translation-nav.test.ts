import { afterEach, describe, expect, it } from "vite-plus/test";

import { hrefFor, liveReaderPosition, positionOf } from "../translation-nav";

describe("reader position parsing", () => {
  it("parses an arabic surah root — one URL per surah, no page tail", () => {
    expect(positionOf("/al-fatihah")).toEqual({ kind: "surah", slug: "al-fatihah" });
  });

  it("parses an arabic global page, juz, and hizb — never a retired rubʿ", () => {
    expect(positionOf("/page/42")).toEqual({ kind: "globalPage", n: 42 });
    expect(positionOf("/juz/30")).toEqual({ kind: "juz", n: 30 });
    expect(positionOf("/hizb/60")).toEqual({ kind: "hizb", n: 60 });
    expect(positionOf("/rub/240")).toBeNull();
  });

  it("parses a translated juz route keeping lang/translator", () => {
    expect(positionOf("/t/ms/basmeih/juz/30")).toEqual({
      kind: "juz",
      n: 30,
      lang: "ms",
      translator: "basmeih",
    });
  });

  it("parses a translated surah and translated hizb keeping lang/translator", () => {
    expect(positionOf("/ar-rum/t/en/sahih")).toEqual({
      kind: "surah",
      slug: "ar-rum",
      lang: "en",
      translator: "sahih",
    });
    expect(positionOf("/t/en/sahih/hizb/7")).toEqual({
      kind: "hizb",
      n: 7,
      lang: "en",
      translator: "sahih",
    });
  });

  it("returns null for the reader home and malformed t routes", () => {
    expect(positionOf("/app")).toBeNull(); // dead legacy marker never parses
    expect(positionOf("/")).toBeNull();
    expect(positionOf("/t/ms")).toBeNull();
  });
});

describe("hrefFor position-preserving translation switch", () => {
  it("rebuilds a juz position for another translation via the ctx-aware helpers", () => {
    const pos = positionOf("/t/ms/basmeih/juz/30");
    expect(hrefFor(pos, { id: "en.sahih", lang: "en", translator: "sahih" })).toBe(
      "/t/en/sahih/juz/30",
    );
  });

  it("rebuilds a hizb position for another translation", () => {
    const hizb = positionOf("/t/ms/basmeih/hizb/3");
    expect(hrefFor(hizb, { id: "ur.jalandhry", lang: "ur", translator: "jalandhry" })).toBe(
      "/t/ur/jalandhry/hizb/3",
    );
  });

  it("rebuilds a surah root for another translation (never a page tail)", () => {
    const pos = positionOf("/ar-rum");
    expect(hrefFor(pos, { id: "ur.jalandhry", lang: "ur", translator: "jalandhry" })).toBe(
      "/ar-rum/t/ur/jalandhry",
    );
  });

  it("rebuilds a global page position", () => {
    const pos = positionOf("/page/42");
    expect(hrefFor(pos, { id: "en.sahih", lang: "en", translator: "sahih" })).toBe(
      "/t/en/sahih/page/42",
    );
  });

  it("returns null when there is no reader position", () => {
    expect(hrefFor(null, { id: "en.sahih", lang: "en", translator: "sahih" })).toBeNull();
  });
});

describe("liveReaderPosition (live url first, page store fallback)", () => {
  afterEach(() => {
    // Tests within a file share the happy-dom window; restore the default
    // non-reader location so later tests stay on the fallback path.
    window.history.replaceState({}, "", "/");
  });

  it("reads the live window.location when it names a reader position", () => {
    window.history.replaceState({}, "", "/al-baqarah");
    expect(liveReaderPosition(new URL("https://example.test/al-fatihah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
    });
  });

  it("de-localizes the live /ar url before parsing it", () => {
    window.history.replaceState({}, "", "/ar/al-baqarah");
    expect(liveReaderPosition(new URL("https://example.test/al-fatihah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
    });
  });

  it("falls back to the passed page-store url when the live url is not a reader route", () => {
    // happy-dom's default location "/" parses to no reader position.
    expect(liveReaderPosition(new URL("https://example.test/juz/30"))).toEqual({
      kind: "juz",
      n: 30,
    });
    expect(liveReaderPosition(new URL("https://example.test/t/ms/basmeih/juz/30"))).toEqual({
      kind: "juz",
      n: 30,
      lang: "ms",
      translator: "basmeih",
    });
  });
});
