import { describe, expect, it } from "vite-plus/test";

import { hrefFor, positionOf } from "../translation-nav";

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
