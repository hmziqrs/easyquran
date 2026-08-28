import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { parseOklch } from "../../lib/theme/derive.ts";

/**
 * Machine guard for the §4 semantic contract (docs/design-system.md): every one of the 8
 * `[data-palette="X"][data-mode="Y"]` blocks in layout.css must define the full token set,
 * and the retired `[data-theme]`/`[data-surface]`/`[data-accent]` selectors must stay gone.
 */
// Read via process.cwd() (vitest runs from web/) — `new URL(literal, import.meta.url)` gets
// rewritten by Vite's asset transform for .css targets and resolves to a non-file URL.
const css = readFileSync(resolve(process.cwd(), "src/routes/layout.css"), "utf8");

const CONTRACT_TOKENS = [
  "--background",
  "--background-subtle",
  "--surface",
  "--surface-raised",
  "--surface-hover",
  "--foreground",
  "--foreground-secondary",
  "--muted",
  "--border",
  "--border-strong",
  "--primary",
  "--primary-hover",
  "--primary-foreground",
  "--primary-soft",
  "--accent",
  "--accent-strong",
  "--accent-soft",
  "--success",
  "--warning",
  "--danger",
  "--focus-ring",
  "--reader-background",
  "--quran-foreground",
  "--translation-foreground",
  "--reader-divider",
] as const;

const PALETTES = ["sacred", "ink", "sepia", "sapphire"] as const;
const MODES = ["light", "dark"] as const;

// Additive hue set (plan 01): the four card hues. Identical across palettes, differs by
// mode only (plan 00 D3 — amber cannot carry white on the dark ground).
const HUE_TOKENS = [
  "--hue-1",
  "--hue-1-soft",
  "--on-hue-1",
  "--hue-2",
  "--hue-2-soft",
  "--on-hue-2",
  "--hue-3",
  "--hue-3-soft",
  "--on-hue-3",
  "--hue-4",
  "--hue-4-soft",
  "--on-hue-4",
  "--hue-1-legible",
  "--hue-2-legible",
  "--hue-3-legible",
  "--hue-4-legible",
] as const;

// Ground tokens (plan 01): identical across palettes within a mode — the check that
// catches a stray tint sneaking back into one palette.
const GROUND_TOKENS = [
  "--background",
  "--background-subtle",
  "--surface",
  "--surface-raised",
  "--surface-hover",
  "--foreground",
  "--foreground-secondary",
  "--muted",
  "--border",
  "--border-strong",
] as const;

function paletteBlock(palette: string, mode: string): string {
  const marker = `[data-palette="${palette}"][data-mode="${mode}"]`;
  const start = css.indexOf(marker);
  expect(start, `missing palette block ${marker}`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  const close = css.indexOf("\n}", open);
  return css.slice(open + 1, close);
}

/** Raw declared value of `token` inside a block, or null when the block omits it. */
function tokenValue(block: string, token: string): string | null {
  const m = new RegExp(`${token}:\\s*([^;]+);`).exec(block);
  return m ? m[1]!.trim() : null;
}

describe("layout.css palette contract", () => {
  for (const palette of PALETTES) {
    for (const mode of MODES) {
      it(`defines the full §4 contract for ${palette} ${mode}`, () => {
        const block = paletteBlock(palette, mode);
        for (const token of CONTRACT_TOKENS) {
          expect(block, `${palette} ${mode} missing ${token}`).toContain(`${token}:`);
        }
      });
    }
  }

  it("defines the additive hue set (plan 01) in all eight blocks", () => {
    for (const palette of PALETTES) {
      for (const mode of MODES) {
        const block = paletteBlock(palette, mode);
        for (const token of HUE_TOKENS) {
          expect(block, `${palette} ${mode} missing ${token}`).toContain(`${token}:`);
        }
      }
    }
  });

  it("shares one hue set across palettes within a mode (only the accent differs)", () => {
    for (const mode of MODES) {
      const sacredBlock = paletteBlock("sacred", mode);
      for (const token of HUE_TOKENS) {
        const expected = tokenValue(sacredBlock, token);
        expect(expected, `sacred ${mode} ${token} has no value`).not.toBeNull();
        for (const palette of PALETTES) {
          if (palette === "sacred") continue;
          expect(tokenValue(paletteBlock(palette, mode), token), `${palette} ${mode} ${token}`).toBe(
            expected,
          );
        }
      }
    }
  });

  it("shares one neutral ground across palettes within a mode (plan 01)", () => {
    for (const mode of MODES) {
      const sacredBlock = paletteBlock("sacred", mode);
      for (const token of GROUND_TOKENS) {
        const expected = tokenValue(sacredBlock, token);
        expect(expected, `sacred ${mode} ${token} has no value`).not.toBeNull();
        for (const palette of PALETTES) {
          if (palette === "sacred") continue;
          expect(tokenValue(paletteBlock(palette, mode), token), `${palette} ${mode} ${token}`).toBe(
            expected,
          );
        }
      }
    }
  });

  it("declares every light --background as zero-chroma oklch (no tint sneaks back in)", () => {
    for (const palette of PALETTES) {
      const raw = tokenValue(paletteBlock(palette, "light"), "--background");
      expect(raw, `${palette} light --background missing`).not.toBeNull();
      const ok = parseOklch(raw!);
      expect(ok, `${palette} light --background is not an oklch() value`).not.toBeNull();
      expect(ok!.c, `${palette} light --background chroma must be 0`).toBe(0);
    }
  });

  it("repoints the retired editorial accent onto the hue-2 family (plan 01)", () => {
    for (const palette of PALETTES) {
      for (const mode of MODES) {
        const block = paletteBlock(palette, mode);
        expect(block, `${palette} ${mode} --accent`).toContain("--accent: var(--hue-2);");
        expect(block, `${palette} ${mode} --accent-strong`).toContain(
          "--accent-strong: var(--hue-2);",
        );
        expect(block, `${palette} ${mode} --accent-soft`).toContain(
          "--accent-soft: var(--hue-2-soft);",
        );
      }
    }
  });

  it("zeroes both elevation tokens in both modes (boards have no shadows)", () => {
    const decls = [...css.matchAll(/--elev-(?:sm|md):\s*([^;]+);/g)];
    expect(decls.length, "expected four --elev-sm/md declarations").toBe(4);
    for (const [, value] of decls) {
      expect(value!.trim()).toBe("none");
    }
  });

  it("keys the dark variant off data-mode, not the retired data-theme", () => {
    expect(css).toContain('@custom-variant dark (&:where([data-mode="dark"]');
    expect(css).not.toContain('data-theme="dark"');
    expect(css).not.toContain("[data-surface=");
    expect(css).not.toContain("[data-accent=");
  });

  it("keeps the legacy alias shim below the palette blocks", () => {
    const aliasAt = css.indexOf("--bg: var(--background);");
    const lastPaletteAt = css.lastIndexOf('[data-palette="sapphire"][data-mode="dark"]');
    expect(aliasAt).toBeGreaterThan(lastPaletteAt);
  });

  it("aliases the legacy interactive accent to the primary family", () => {
    expect(css).toContain("--color-accent: var(--primary);");
    expect(css).toContain("--color-accent-fg: var(--accent-fg);");
    expect(css).toContain("--accent-fg: var(--primary-foreground);");
    expect(css).toContain("--color-pop: var(--accent);");
    expect(css).toContain("--color-ok: var(--success);");
    expect(css).toContain("--color-muted-foreground: var(--muted);");
  });

  it("keeps the border utility alive (border-border/divide-border consumers)", () => {
    expect(css).toContain("--color-border: var(--border);");
  });

  it("maps text-muted to the foreground-side --muted token, never a surface (§4/§9)", () => {
    expect(css).toContain("--color-muted: var(--muted);");
    expect(css).not.toContain("--color-muted: var(--surface-hover);");
  });
});
