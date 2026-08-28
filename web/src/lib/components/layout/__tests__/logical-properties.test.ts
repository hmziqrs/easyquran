import { readFileSync, readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

/**
 * Machine guard for the plan 04 RTL contract (docs/plan/04-layout-full-bleed.md):
 * ported styles use logical properties — ms-/me-/ps-/pe-/start-/end-/text-start —
 * never the physical ml-/mr-/pl-/pr-/left-N/right-N/text-left/border-l/border-r.
 *
 * Scoped to the band-migrated surfaces (routes/(marketing) + lib/components/layout)
 * so it lands green; widen as more surfaces port.
 *
 * Two LEGITIMATE physical uses are exempt by pattern, not by silence:
 * - `left-1/2`/`right-1/2` paired with a translate — horizontal CENTERING is
 *   direction-symmetric; converting it to inset-inline breaks it under RTL.
 * - `data-[side=left|right]:…` in sheet/sidebar primitives — "side" is the physical
 *   screen edge that side names (none of these live in the guarded dirs today).
 */

const read = (path: string): string => readFileSync(resolve(process.cwd(), path), "utf8");

/** Recursively collect files under a directory matching a suffix. */
function walk(dir: string, suffix: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(resolve(process.cwd(), dir))) {
    const full = `${dir}/${entry}`;
    if (statSync(resolve(process.cwd(), full)).isDirectory()) out.push(...walk(full, suffix));
    else if (entry.endsWith(suffix)) out.push(full);
  }
  return out;
}

/* Each pattern: what it bans, and why it cannot hit a logical class.
   `left-1/2` (centering) is carved out by the (?!\/2) lookahead. `border-[lr]`
   must not match `border-line*` colours, hence the trailing boundary. */
const PHYSICAL: [name: string, re: RegExp][] = [
  ["ml-/mr-", /\bm[lr]-/],
  ["pl-/pr-", /\bp[lr]-/],
  ["text-left/text-right", /\btext-(?:left|right)\b/],
  ["left-N/right-N (centering halves exempt)", /\b(?:left|right)-\d(?!\/2)/],
  ["border-l/border-r", /\bborder-[lr](?:-\d)?(?![-\w])/],
];

const guarded = [
  ...walk("src/lib/components/layout", ".svelte"),
  ...walk("src/routes/(marketing)", ".svelte"),
];

describe("plan 04 logical properties guard", () => {
  it("found the guarded trees (sanity — not vacuous)", () => {
    expect(guarded.filter((f) => f.startsWith("src/lib/components/layout")).length).toBe(3);
    expect(guarded.filter((f) => f.startsWith("src/routes/(marketing)")).length).toBeGreaterThan(5);
  });

  for (const file of guarded) {
    for (const [name, re] of PHYSICAL) {
      it(`${file} uses no ${name}`, () => {
        expect(read(file).match(re), `${file}: ${String(read(file).match(re))}`).toBeNull();
      });
    }
  }
});

describe("plan 04 band model", () => {
  const band = read("src/lib/components/layout/Band.svelte");
  const section = read("src/lib/components/layout/Section.svelte");

  it("Band owns its container — it neither imports nor renders a Container", () => {
    expect(band).not.toMatch(/import\s+Container|<Container/);
  });

  it("Band paints tones via lookup tables and the §15 ramp, not per-breakpoint classes", () => {
    expect(band).toContain("px-(--gutter)");
    expect(band).toContain("py-(--band-pad)");
    expect(band).toContain("py-(--band-pad-tight)");
    expect(band).not.toMatch(/\b(?:md|lg|xl):/);
  });

  it("Section stays output-identical over Band: page tone, no band pad, legacy px-6 gutter", () => {
    expect(section).toContain('tone="page"');
    expect(section).toContain('pad="none"');
    expect(section).toContain('contentClass="px-6"');
    expect(section).toContain('tight ? "py-12" : "py-16 md:py-24"');
  });

  it("layout.css carries the responsive ramp ladder (20/32/48/72 gutter, 48→96 pad)", () => {
    const css = read("src/routes/layout.css");
    expect(css).toContain("--gutter: 20px;");
    expect(css.match(/--gutter: 32px;/)).not.toBeNull();
    expect(css.match(/--gutter: 48px;/)).not.toBeNull();
    expect(css.match(/--gutter: 72px;/)).not.toBeNull();
    expect(css.match(/--band-pad: 48px;/)).not.toBeNull();
    expect(css.match(/--band-pad: 64px;/)).not.toBeNull();
    expect(css.match(/--band-pad: 80px;/)).not.toBeNull();
    expect(css.match(/--band-pad: 96px;/)).not.toBeNull();
    expect(css.match(/--band-pad-tight: 64px;/)).not.toBeNull();
  });
});
