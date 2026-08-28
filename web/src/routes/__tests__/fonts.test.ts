import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

/**
 * The fonts gate (docs/plan/02-typography-and-fonts.md). Fonts are self-hosted only —
 * a CDN reference would break offline reading, which is a functional bug, not a style
 * one. This guard converts that risk into a build failure, and pins the Nunito ramp
 * (one Latin face, 600 body / 800 headings) so areas 3–6 can consume it as a contract.
 */
// Read via process.cwd() (vitest runs from web/) — `new URL(literal, import.meta.url)` gets
// rewritten by Vite's asset transform for .css targets and resolves to a non-file URL.
const css = readFileSync(resolve(process.cwd(), "src/routes/layout.css"), "utf8");
const pkg = readFileSync(resolve(process.cwd(), "package.json"), "utf8");

/** Plan 02 ramp: role → [size, line-height, weight]. Aliased roles assert their alias. */
const RAMP = {
  "--text-display-xl": ["76px", "1.06", "800"],
  "--text-h1": ["40px", "1.1", "800"],
  "--text-h2": ["26px", "1.2", "800"],
  "--text-h3": ["20px", "1.25", "800"],
  "--text-body-xl": ["20px", "1.55", "600"],
  "--text-body-l": ["17.5px", "1.6", "600"],
  "--text-body": ["15px", "1.5", "600"],
  "--text-caption": ["13.5px", "1.45", "600"],
  "--text-micro": ["13px", "1.4", "800"],
} as const;

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

  it("--font-sans resolves to the self-hosted Nunito family", () => {
    const sans = tokenValue("--font-sans", "");
    expect(sans).toContain('"Nunito Variable"');
    expect(sans).toContain("ui-rounded");
  });

  it("Inter and Newsreader are gone from layout.css and package.json", () => {
    for (const hay of [css, pkg]) {
      expect(hay).not.toMatch(/fontsource-variable\/inter/u);
      expect(hay).not.toMatch(/newsreader/ui);
      expect(hay).not.toMatch(/Inter Variable/u);
    }
    expect(pkg).toMatch(/@fontsource-variable\/nunito/u);
  });

  it("--font-display is retired (no token, no consumer)", () => {
    expect(css).not.toMatch(/--font-display/u);
    expect(css).not.toMatch(/font-serif/u);
  });

  it("every ramp role defines size, line-height and weight with the plan 02 values", () => {
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
    expect(tokenValue("--text-micro", "letter-spacing")).toBe("0.1em");
  });

  it("Arabic optical ratio is encoded and consumed by the .arabic utility", () => {
    expect(tokenValue("--font-size-arabic-ratio", "")).toBe("1.4");
    expect(css).toMatch(/@utility\s+arabic\s*\{/u);
    expect(css).toMatch(/font-size:\s*calc\(var\(--font-size-arabic-ratio\)\s*\*\s*1em\)/u);
  });
});
