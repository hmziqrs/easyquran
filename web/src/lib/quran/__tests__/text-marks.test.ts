import { describe, expect, it } from "vite-plus/test";

import {
  isTafsirTranslation,
  parseTransliterationSegments,
  stripTransliterationMarkup,
} from "#lib/quran/text-marks.js";

describe("isTafsirTranslation (TRX-01 frozen allow-list)", () => {
  it("marks exactly the two Arabic tafsir works filed among translations", () => {
    expect(isTafsirTranslation("ar.jalalayn")).toBe(true);
    expect(isTafsirTranslation("ar.muyassar")).toBe(true);
    expect(isTafsirTranslation("en.sahih")).toBe(false);
    // Lookalikes and prefixes must not leak commentary into the kind test.
    expect(isTafsirTranslation("ar")).toBe(false);
    expect(isTafsirTranslation("qul.ar.muyassar")).toBe(false);
    expect(isTafsirTranslation("ar.muyassar.v2")).toBe(false);
  });
});

describe("parseTransliterationSegments (TRX-02)", () => {
  it("returns one unmarked segment for plain text", () => {
    expect(parseTransliterationSegments("In the name of God")).toEqual([
      { text: "In the name of God", underline: false, strong: false },
    ]);
  });

  it("parses underline and strong runs with case-insensitive closers", () => {
    expect(parseTransliterationSegments("Bismi All<u>a</U>hi a<b>l</B>rra<u>h</U>m")).toEqual([
      { text: "Bismi All", underline: false, strong: false },
      { text: "a", underline: true, strong: false },
      { text: "hi a", underline: false, strong: false },
      { text: "l", underline: false, strong: true },
      { text: "rra", underline: false, strong: false },
      { text: "h", underline: true, strong: false },
      { text: "m", underline: false, strong: false },
    ]);
  });

  it("passes a stray < through verbatim instead of dropping it", () => {
    expect(parseTransliterationSegments("a < b")).toEqual([
      { text: "a < b", underline: false, strong: false },
    ]);
  });
});

describe("stripTransliterationMarkup (TRX-02 copy path)", () => {
  it("removes the tags and keeps every letter", () => {
    expect(stripTransliterationMarkup("Bismi All<u>a</U>hi a<b>l</B>rra<u>h</U>m")).toBe(
      "Bismi Allahi alrrahm",
    );
  });

  it("leaves plain text untouched", () => {
    expect(stripTransliterationMarkup("a < b")).toBe("a < b");
  });
});
