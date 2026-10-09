import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

/**
 * The fonts gate (docs/plan/README.md (phase specs in git history)). Fonts are self-hosted only —
 * a CDN reference would break offline reading, which is a functional bug, not a style
 * one. This guard converts that risk into a build failure, and pins the Onest ramp
 * (one Latin face; hmziq brand kit weights: 400 text, 500 headings, nothing above 600)
 * so areas 3–6 can consume it as a contract.
 */
// Read via process.cwd() (vitest runs from web/) — `new URL(literal, import.meta.url)` gets
// rewritten by Vite's asset transform for .css targets and resolves to a non-file URL.
const css = readFileSync(resolve(process.cwd(), "src/routes/layout.css"), "utf8");
const pkg = readFileSync(resolve(process.cwd(), "package.json"), "utf8");

/** Ramp: role → [size, line-height, weight]. Sizes/line-heights from the plan 02 boards,
 *  weights from the brand kit. Aliased roles assert their alias. */
const RAMP = {
  "--text-display-xl": ["76px", "1.06", "500"],
  "--text-h1": ["40px", "1.1", "500"],
  "--text-h2": ["26px", "1.2", "500"],
  "--text-h3": ["20px", "1.25", "500"],
  "--text-body-xl": ["20px", "1.55", "400"],
  "--text-body-l": ["17.5px", "1.6", "400"],
  "--text-body": ["15px", "1.5", "400"],
  "--text-caption": ["13.5px", "1.45", "400"],
  "--text-micro": ["13px", "1.4", "500"],
} as const;

/** Brand kit ceiling: nothing heavier than 600 (the wordmark). */
const MAX_WEIGHT = 600;

// Retired roles (plan 02): kept as aliases of their nearest neighbour for one release.
const ALIASES = {
  "--text-display-l": "--text-h1",
  "--text-body-s": "--text-body",
} as const;

// Built from pieces so this file never contains the contiguous literal — otherwise the
// repo grep gate (`rg "fonts\.(googleapis|gstatic)" web/src web/static`) and this test's
// own directory walk would both flag the guard itself.
const CDN_PATTERN = new RegExp("fonts\\." + "(googleapis|gstatic)");

function tokenValue(name: string, suffix: string): string | undefined {
  // Line-anchored: a bare `--text-h1:` must not match the `--text-h1--line-height:` line,
  // and values inside var() aliases (never at line start) are never picked up.
  const decl = suffix === "" ? `${name}\\s*:` : `${name}--${suffix}\\s*:`;
  const match = css.match(new RegExp(`^\\s*${decl}([^;]+);`, "mu"));
  return match?.[1]?.trim();
}

/** Textual files under `dir`, recursively — binaries are skipped by extension. */
function textFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...textFiles(full));
    } else if (/\.(?:ts|js|mjs|svelte|css|json|html|xml|txt|webmanifest)$/u.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

describe("fonts gate (plan 02)", () => {
  it("layout.css has no CDN font imports", () => {
    expect(css).not.toMatch(CDN_PATTERN);
    expect(css).not.toMatch(/@import\s+url\(\s*https?:/u);
  });

  it("no source or static file references the Google Fonts CDN", () => {
    const roots = ["src", "static"].map((r) => resolve(process.cwd(), r));
    const offenders = roots.flatMap((root) =>
      textFiles(root).filter((f) => CDN_PATTERN.test(readFileSync(f, "utf8"))),
    );
    expect(offenders).toEqual([]);
  });

  it("--font-sans resolves to the self-hosted Onest family", () => {
    const sans = tokenValue("--font-sans", "");
    expect(sans).toMatch(/^"Onest Variable"/u);
    expect(sans).not.toMatch(/Nunito|ui-rounded/u);
    expect(css).toMatch(/@import\s+"@fontsource-variable\/onest"/u);
  });

  it("--font-mono resolves to the self-hosted JetBrains Mono family", () => {
    const mono = tokenValue("--font-mono", "");
    expect(mono).toMatch(/^"JetBrains Mono Variable"/u);
    expect(css).toMatch(/@import\s+"@fontsource-variable\/jetbrains-mono"/u);
    expect(css).not.toMatch(/geist-mono/iu);
  });

  it("Inter and Newsreader are gone from layout.css and package.json", () => {
    for (const hay of [css, pkg]) {
      expect(hay).not.toMatch(/fontsource-variable\/inter/u);
      expect(hay).not.toMatch(/newsreader/iu);
      expect(hay).not.toMatch(/Inter Variable/u);
    }
    expect(pkg).toMatch(/@fontsource-variable\/onest/u);
  });

  it("--font-display is retired (no token, no consumer)", () => {
    expect(css).not.toMatch(/--font-display/u);
    expect(css).not.toMatch(/font-serif/u);
  });

  it("every ramp role defines size, line-height and weight with the ramp values", () => {
    for (const [role, [size, lineHeight, weight]] of Object.entries(RAMP)) {
      expect(tokenValue(role, ""), `${role} size`).toBe(size);
      expect(tokenValue(role, "line-height"), `${role} line-height`).toBe(lineHeight);
      expect(tokenValue(role, "font-weight"), `${role} weight`).toBe(weight);
    }
  });

  it("retired roles alias their nearest neighbour (size, line-height, weight)", () => {
    for (const [alias, target] of Object.entries(ALIASES)) {
      expect(tokenValue(alias, ""), `${alias} size`).toBe(`var(${target})`);
      expect(tokenValue(alias, "line-height"), `${alias} line-height`).toBe(
        `var(${target}--line-height)`,
      );
      expect(tokenValue(alias, "font-weight"), `${alias} weight`).toBe(
        `var(${target}--font-weight)`,
      );
    }
  });

  it("ramp tracking is encoded on the roles (plan 02 / boards)", () => {
    expect(tokenValue("--text-display-xl", "letter-spacing")).toBe("-0.04em");
    expect(tokenValue("--text-h1", "letter-spacing")).toBe("-0.035em");
    expect(tokenValue("--text-h2", "letter-spacing")).toBe("-0.03em");
    expect(tokenValue("--text-h3", "letter-spacing")).toBe("-0.025em");
    // No spaced-out labels (brand kit): micro tracks at 0 like every role below 20px.
    expect(tokenValue("--text-micro", "letter-spacing")).toBe("0");
  });

  it("no app source asks for a weight above 600 (bold/extrabold/black, 700–900)", () => {
    // The design lab (/design) keeps its historical studies; everything else is the site.
    const heavy = /\bfont-(?:bold|extrabold|black)\b|font-weight:\s*[7-9]00\b/u;
    const offenders = textFiles(resolve(process.cwd(), "src"))
      .filter((f) => !f.includes(join("routes", "design")) && !f.includes("__tests__"))
      .filter((f) => heavy.test(readFileSync(f, "utf8")));
    expect(offenders).toEqual([]);
  });

  it("no declared weight in layout.css exceeds the brand kit ceiling", () => {
    const weights = [...css.matchAll(/font-weight:\s*(\d{3})\s*;/gu)].map((m) => Number(m[1]));
    expect(weights.length).toBeGreaterThan(0);
    expect(weights.filter((w) => w > MAX_WEIGHT)).toEqual([]);
  });

  it("Arabic optical ratio is encoded and consumed by the .arabic utility", () => {
    expect(tokenValue("--font-size-arabic-ratio", "")).toBe("1.45");
    expect(css).toMatch(/@utility\s+arabic\s*\{/u);
    expect(css).toMatch(/font-size:\s*calc\(var\(--font-size-arabic-ratio\)\s*\*\s*1em\)/u);
  });
});
