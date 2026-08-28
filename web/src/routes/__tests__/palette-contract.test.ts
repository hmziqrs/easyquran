import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

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

function paletteBlock(palette: string, mode: string): string {
  const marker = `[data-palette="${palette}"][data-mode="${mode}"]`;
  const start = css.indexOf(marker);
  expect(start, `missing palette block ${marker}`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  const close = css.indexOf("\n}", open);
  return css.slice(open + 1, close);
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
