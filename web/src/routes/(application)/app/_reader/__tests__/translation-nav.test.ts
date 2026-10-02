import { afterEach, describe, expect, it } from "vite-plus/test";

import { hrefFor, liveReaderPosition, positionOf } from "../translation-nav";

describe("reader position parsing", () => {
  it("parses an arabic surah root — one URL per surah, no page tail", () => {
    expect(positionOf("/app/al-fatihah")).toEqual({ kind: "surah", slug: "al-fatihah" });
  });

  it("parses an arabic global page, juz, hizb, and rub", () => {
    expect(positionOf("/app/page/42")).toEqual({ kind: "globalPage", n: 42 });
    expect(positionOf("/app/juz/30")).toEqual({ kind: "juz", n: 30 });
    expect(positionOf("/app/hizb/60")).toEqual({ kind: "hizb", n: 60 });
    expect(positionOf("/app/rub/240")).toEqual({ kind: "rub", n: 240 });
  });

  it("parses a translated juz route keeping lang/translator", () => {
    expect(positionOf("/app/t/ms/basmeih/juz/30")).toEqual({
      kind: "juz",
      n: 30,
      lang: "ms",
      translator: "basmeih",
    });
  });

  it("parses a translated surah and translated hizb keeping lang/translator", () => {
    expect(positionOf("/app/ar-rum/t/en/sahih")).toEqual({
      kind: "surah",
      slug: "ar-rum",
      lang: "en",
      translator: "sahih",
    });
    expect(positionOf("/app/t/en/sahih/hizb/7")).toEqual({
      kind: "hizb",
      n: 7,
      lang: "en",
      translator: "sahih",
    });
  });

  it("returns null for the reader home and malformed t routes", () => {
    expect(positionOf("/app")).toBeNull();
    expect(positionOf("/app/t/ms")).toBeNull();
  });
});

describe("hrefFor position-preserving translation switch", () => {
  it("rebuilds a juz position for another translation via the ctx-aware helpers", () => {
    const pos = positionOf("/app/t/ms/basmeih/juz/30");
    expect(hrefFor(pos, { id: "en.sahih", lang: "en", translator: "sahih" })).toBe(
      "/app/t/en/sahih/juz/30",
    );
  });

  it("rebuilds a hizb and rub position for another translation", () => {
    const hizb = positionOf("/app/t/ms/basmeih/hizb/3");
    expect(hrefFor(hizb, { id: "ur.jalandhry", lang: "ur", translator: "jalandhry" })).toBe(
      "/app/t/ur/jalandhry/hizb/3",
    );
    const rub = positionOf("/app/rub/9");
    expect(hrefFor(rub, { id: "en.sahih", lang: "en", translator: "sahih" })).toBe(
      "/app/t/en/sahih/rub/9",
    );
  });

  it("rebuilds a surah root for another translation (never a page tail)", () => {
    const pos = positionOf("/app/ar-rum");
    expect(hrefFor(pos, { id: "ur.jalandhry", lang: "ur", translator: "jalandhry" })).toBe(
      "/app/ar-rum/t/ur/jalandhry",
    );
  });

  it("rebuilds a global page position", () => {
    const pos = positionOf("/app/page/42");
    expect(hrefFor(pos, { id: "en.sahih", lang: "en", translator: "sahih" })).toBe(
      "/app/t/en/sahih/page/42",
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
    window.history.replaceState({}, "", "/app/al-baqarah");
    expect(liveReaderPosition(new URL("https://example.test/app/al-fatihah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
    });
  });

  it("de-localizes the live url before parsing it", () => {
    window.history.replaceState({}, "", "/en/app/al-baqarah");
    expect(liveReaderPosition(new URL("https://example.test/app/al-fatihah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
    });
  });

  it("falls back to the passed page-store url when the live url is not a reader route", () => {
    // happy-dom's default location "/" parses to no reader position.
    expect(liveReaderPosition(new URL("https://example.test/app/juz/30"))).toEqual({
      kind: "juz",
      n: 30,
    });
    expect(liveReaderPosition(new URL("https://example.test/app/t/ms/basmeih/juz/30"))).toEqual({
      kind: "juz",
      n: 30,
      lang: "ms",
      translator: "basmeih",
    });
  });
});
