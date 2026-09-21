import { afterEach, describe, expect, it } from "vite-plus/test";

import { hrefFor, liveReaderPosition, positionOf } from "../translation-nav";

describe("reader position parsing", () => {
  it("parses an arabic surah page", () => {
    expect(positionOf("/app/al-fatihah/page/3")).toEqual({
      kind: "surah",
      slug: "al-fatihah",
      localPage: 3,
    });
  });

  it("parses an arabic global page and juz", () => {
    expect(positionOf("/app/page/42")).toEqual({ kind: "globalPage", n: 42 });
    expect(positionOf("/app/juz/30")).toEqual({ kind: "juz", n: 30 });
  });

  it("parses a translated juz route keeping lang/translator", () => {
    expect(positionOf("/app/t/ms/basmeih/juz/30")).toEqual({
      kind: "juz",
      n: 30,
      lang: "ms",
      translator: "basmeih",
    });
  });

  it("parses a surah-local translated route", () => {
    expect(positionOf("/app/ar-rum/t/en/sahih/page/2")).toEqual({
      kind: "surah",
      slug: "ar-rum",
      localPage: 2,
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

  it("rebuilds a surah local page for another translation", () => {
    const pos = positionOf("/app/ar-rum/page/2");
    expect(hrefFor(pos, { id: "ur.jalandhry", lang: "ur", translator: "jalandhry" })).toBe(
      "/app/ar-rum/t/ur/jalandhry/page/2",
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

describe("liveReaderPosition (stress S1: live url first, page store fallback)", () => {
  afterEach(() => {
    // Tests within a file share the happy-dom window; restore the default
    // non-reader location so later tests stay on the fallback path.
    window.history.replaceState({}, "", "/");
  });

  it("reads the live window.location when it names a reader position", () => {
    // Scrolled surah route: the reader's scroll handler rewrote the live url
    // to /page/N while the page store (fallbackUrl here) still holds the bare
    // surah slug — SvelteKit 2.70.2 replaceState never updates page.url.
    window.history.replaceState({}, "", "/app/al-baqarah/page/2");
    expect(liveReaderPosition(new URL("https://example.test/app/al-baqarah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
      localPage: 2,
    });
  });

  it("de-localizes the live url before parsing it", () => {
    window.history.replaceState({}, "", "/en/app/al-baqarah/page/3");
    expect(liveReaderPosition(new URL("https://example.test/app/al-baqarah"))).toEqual({
      kind: "surah",
      slug: "al-baqarah",
      localPage: 3,
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
