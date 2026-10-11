import { describe, expect, it } from "vite-plus/test";

import {
  contrastRatio,
  deriveTokens,
  isLight,
  luminance,
  oklchToRgb,
  parseColor,
  parseHex,
  parseOklch,
  tokensToCss,
  toHex,
} from "../derive";

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
    expect(lum(t["--bg-elev"]!)).toBeLessThan(lum("#101820"));
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

  it("emits the §4 contract alongside the legacy ramp, consistently", () => {
    const t = deriveTokens({ bg: "#101820" });
    // Plan 01: the ground is neutral — a chromatic seed is quantised to its grey twin, so
    // --background keeps the seed's luminance but loses its tint entirely.
    const bg = parseHex(t["--background"]!)!;
    expect(bg.r).toBe(bg.g);
    expect(bg.g).toBe(bg.b);
    expect(luminance(bg)).toBeCloseTo(lum("#101820"), 2);
    expect(t["--foreground"]).toBe(t["--fg"]);
    expect(t["--foreground-secondary"]).toBe(t["--fg-2"]);
    expect(t["--muted"]).toBe(t["--fg-3"]);
    expect(t["--border"]).toBe(t["--line"]);
    expect(t["--border-strong"]).toBe(t["--line-2"]);
    expect(lum(t["--surface"]!)).toBeGreaterThan(lum(t["--background"]!));
    expect(lum(t["--surface-raised"]!)).toBeGreaterThan(lum(t["--surface"]!));
    // §42: the reader sits darker than the app background at night.
    expect(lum(t["--reader-background"]!)).toBeLessThan(lum(t["--background"]!));
  });

  it("produces zero-chroma ground ramps from any seed (plan 01 neutral ground)", () => {
    const hexGroundTokens = [
      "--background",
      "--background-subtle",
      "--surface",
      "--surface-raised",
      "--surface-hover",
      "--foreground",
      "--foreground-secondary",
      "--muted",
      "--reader-background",
    ];
    for (const seed of ["#808080", "#3fbfa6", "#f4ecd8", "#0b1018"]) {
      const t = deriveTokens({ bg: seed });
      for (const token of hexGroundTokens) {
        const c = parseHex(t[token]!)!;
        expect(c.r, `${token} from ${seed}`).toBe(c.g);
        expect(c.g, `${token} from ${seed}`).toBe(c.b);
      }
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

  it("maps the interactive seed onto the primary family", () => {
    const t = deriveTokens({ accent: "#3fbfa6" });
    expect(t["--primary"]).toBe("#3fbfa6");
    expect(t["--focus-ring"]).toBe("#3fbfa6");
    expect(t["--primary-foreground"]).toBe("#ffffff");
  });

  it("derives --on-hue-1 by luminance, never assumes white (plan 01)", () => {
    // A bright-yellow accent seed is light: its on-colour must be dark.
    const yellow = deriveTokens({ accent: "#f5d76e" });
    expect(lum(yellow["--on-hue-1"]!)).toBeLessThan(0.1);
    expect(yellow["--on-hue-1"]).toBe(yellow["--primary-foreground"]);
    // A deep accent seed takes white.
    expect(deriveTokens({ accent: "#3b2a1a" })["--on-hue-1"]).toBe("#ffffff");
  });
});

describe("deriveTokens — pop", () => {
  it("maps the pop seed onto the editorial accent family", () => {
    const t = deriveTokens({ pop: "#d9af6a" });
    expect(t["--accent"]).toBe("#d9af6a");
    expect(t["--accent-strong"]).toBe("#d9af6a");
    expect(t["--pop"]).toBe("#d9af6a");
    expect(t["--pop-soft"]).toBe("rgba(217, 175, 106, 0.13)");
  });
});

describe("deriveTokens — partial seeds", () => {
  it("contributes nothing for seeds that are absent or unparseable", () => {
    expect(deriveTokens({})).toEqual({});
    expect(deriveTokens({ accent: "not-a-colour" })).toEqual({});
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

describe("parseOklch", () => {
  it("accepts plain numbers, percentages, and hue units", () => {
    expect(parseOklch("oklch(0.52 0.21 262)")).toEqual({ l: 0.52, c: 0.21, h: 262 });
    expect(parseOklch("oklch(52% 0.21 262deg)")).toEqual({ l: 0.52, c: 0.21, h: 262 });
    expect(parseOklch("oklch(0.5 0 0)")).toEqual({ l: 0.5, c: 0, h: 0 });
  });

  it("treats none as zero on any channel", () => {
    expect(parseOklch("oklch(none 0.2 262)")).toEqual({ l: 0, c: 0.2, h: 262 });
    expect(parseOklch("oklch(0.52 0.21 none)")).toEqual({ l: 0.52, c: 0.21, h: 0 });
  });

  it("normalises radian, gradian and turn hues to degrees", () => {
    expect(parseOklch("oklch(0.5 0.1 1.5707963267948966rad)")?.h).toBeCloseTo(90, 5);
    expect(parseOklch("oklch(0.5 0.1 100grad)")?.h).toBeCloseTo(90, 5);
    expect(parseOklch("oklch(0.5 0.1 0.25turn)")?.h).toBeCloseTo(90, 5);
  });

  it("rejects anything else rather than guessing", () => {
    expect(parseOklch("oklch(0.52 0.21 262 / 0.5)")).toBeNull(); // alpha: composite unknown
    expect(parseOklch("oklch(0.52)")).toBeNull();
    expect(parseOklch("#ffffff")).toBeNull();
    expect(parseOklch("")).toBeNull();
  });
});

describe("oklchToRgb / parseColor", () => {
  it("hits the sRGB primaries through the OKLab matrices", () => {
    expect(toHex(oklchToRgb({ l: 1, c: 0, h: 0 }))).toBe("#ffffff");
    expect(toHex(oklchToRgb({ l: 0, c: 0, h: 0 }))).toBe("#000000");
    // Chroma 0 must stay achromatic on every channel — the neutral-ground invariant.
    for (const l of [0.165, 0.52, 0.885, 0.98]) {
      const grey = oklchToRgb({ l, c: 0, h: 0 });
      expect(grey.r).toBe(grey.g);
      expect(grey.g).toBe(grey.b);
    }
  });

  it("converts the plan 01 accents close to their hex neighbours", () => {
    // Cobalt oklch(0.52 0.21 262) sits near #2f5fe0; assert the achromatic anchor instead of
    // an exact hex (matrix rounding shifts the exact value by design).
    const cobalt = oklchToRgb({ l: 0.52, c: 0.21, h: 262 });
    expect(cobalt.b).toBeGreaterThan(cobalt.r);
    expect(luminance(cobalt)).toBeGreaterThan(0.05);
  });

  it("clamps out-of-gamut colors into range instead of wrapping", () => {
    const neon = oklchToRgb({ l: 0.95, c: 0.32, h: 110 });
    for (const v of [neon.r, neon.g, neon.b]) {
      expect(v).toBeLessThanOrEqual(255);
      expect(v).toBeGreaterThanOrEqual(0);
    }
  });

  it("parseColor accepts hex and oklch, rejects the rest", () => {
    expect(toHex(parseColor("#3fbfa6")!)).toBe("#3fbfa6");
    expect(toHex(parseColor("oklch(1 0 0)")!)).toBe("#ffffff");
    expect(parseColor("var(--primary)")).toBeNull();
    expect(parseColor("rgb(1 2 3)")).toBeNull();
  });
});

describe("contrastRatio", () => {
  it("computes the WCAG extremes", () => {
    expect(contrastRatio(parseHex("#000000")!, parseHex("#ffffff")!)).toBeCloseTo(21, 0);
    expect(contrastRatio(parseHex("#ffffff")!, parseHex("#ffffff")!)).toBeCloseTo(1, 10);
  });

  it("is symmetric in its arguments", () => {
    const a = parseHex("#18211D")!;
    const b = parseHex("#F8F7F2")!;
    expect(contrastRatio(a, b)).toBe(contrastRatio(b, a));
  });

  it("keeps white readable on the plan 01 cobalt accent", () => {
    const accent = oklchToRgb({ l: 0.52, c: 0.21, h: 262 });
    expect(contrastRatio(parseHex("#ffffff")!, accent)).toBeGreaterThanOrEqual(4.5);
  });
});
