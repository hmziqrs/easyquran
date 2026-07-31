/* The derivation is what makes a three-colour picker produce a usable theme,
   so the properties worth pinning are the ones a user would notice breaking:
   surfaces must step the RIGHT WAY for the seed's lightness, text must land on
   the opposite end, and accent foregrounds must stay readable. */

import { describe, expect, it } from "vite-plus/test";
import { deriveTokens, isLight, luminance, parseHex, tokensToCss, toHex } from "../derive";

const lum = (hex: string) => luminance(parseHex(hex)!);

describe("parseHex", () => {
  it("accepts 3- and 6-digit forms, with or without the hash", () => {
    expect(parseHex("#fff")).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHex("000000")).toEqual({ r: 0, g: 0, b: 0 });
    expect(parseHex("#3FBFA6")).toEqual({ r: 63, g: 191, b: 166 });
  });

  it("rejects anything else rather than guessing", () => {
    expect(parseHex("rgb(1,2,3)")).toBeNull();
    expect(parseHex("#12345")).toBeNull();
    expect(parseHex("")).toBeNull();
  });
});

describe("deriveTokens — background ramp", () => {
  it("steps surfaces toward white and text toward white on a dark seed", () => {
    const t = deriveTokens({ bg: "#101820" });
    expect(lum(t["--bg-1"]!)).toBeGreaterThan(lum("#101820"));
    expect(lum(t["--bg-2"]!)).toBeGreaterThan(lum(t["--bg-1"]!));
    expect(lum(t["--bg-3"]!)).toBeGreaterThan(lum(t["--bg-2"]!));
    // the nav/footer bar sinks BELOW the page on a dark theme
    expect(lum(t["--bg-elev"]!)).toBeLessThan(lum("#101820"));
    // text ramps from brightest (--fg) down to the muted label step
    expect(lum(t["--fg"]!)).toBeGreaterThan(lum(t["--fg-2"]!));
    expect(lum(t["--fg-2"]!)).toBeGreaterThan(lum(t["--fg-3"]!));
    expect(lum(t["--fg-3"]!)).toBeGreaterThan(lum(t["--fg-4"]!));
  });

  it("reverses both ramps on a light seed", () => {
    const t = deriveTokens({ bg: "#faf6ef" });
    expect(lum(t["--bg-2"]!)).toBeLessThan(lum("#faf6ef"));
    expect(lum(t["--bg-3"]!)).toBeLessThan(lum(t["--bg-2"]!));
    expect(lum(t["--bg-elev"]!)).toBeGreaterThan(lum("#faf6ef"));
    expect(lum(t["--fg"]!)).toBeLessThan(lum(t["--fg-4"]!));
  });

  it("keeps --fg well clear of --bg in both directions", () => {
    for (const seed of ["#000000", "#101820", "#faf6ef", "#ffffff"]) {
      const t = deriveTokens({ bg: seed });
      expect(Math.abs(lum(t["--fg"]!) - lum(seed))).toBeGreaterThan(0.5);
    }
  });
});

describe("deriveTokens — accent", () => {
  it("puts white on a dark accent and a dark tint on a light one", () => {
    expect(deriveTokens({ accent: "#3b2a1a" })["--accent-fg"]).toBe("#ffffff");
    expect(lum(deriveTokens({ accent: "#f5d76e" })["--accent-fg"]!)).toBeLessThan(0.1);
  });

  it("derives the soft/line washes and the focus ring from the same seed", () => {
    const t = deriveTokens({ accent: "#3fbfa6" });
    expect(t["--accent-soft"]).toBe("rgba(63, 191, 166, 0.13)");
    expect(t["--accent-line"]).toBe("rgba(63, 191, 166, 0.32)");
    expect(t["--ring"]).toBe("#3fbfa6");
  });
});

describe("deriveTokens — partial seeds", () => {
  it("contributes nothing for seeds that are absent or unparseable", () => {
    expect(deriveTokens({})).toEqual({});
    expect(deriveTokens({ accent: "not-a-colour" })).toEqual({});
    // an accent-only custom layer must not touch the background family
    expect(Object.keys(deriveTokens({ accent: "#3fbfa6" }))).not.toContain("--bg");
  });
});

describe("helpers", () => {
  it("round-trips through toHex", () => {
    expect(toHex(parseHex("#3fbfa6")!)).toBe("#3fbfa6");
  });

  it("classifies light and dark", () => {
    expect(isLight(parseHex("#ffffff")!)).toBe(true);
    expect(isLight(parseHex("#101820")!)).toBe(false);
  });

  it("emits a paste-ready rule under the given selector", () => {
    const css = tokensToCss({ "--accent": "#fff" }, '[data-theme="dark"]');
    expect(css).toBe('[data-theme="dark"] {\n  --accent: #fff;\n}');
  });
});
