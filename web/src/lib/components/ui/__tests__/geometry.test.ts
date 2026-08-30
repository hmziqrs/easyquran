import { readFileSync, readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

/**
 * Machine guard for the plan 03 geometry contract (docs/plan/README.md (phase specs in git history)):
 * the radius scale, controls-are-pills, icon-holders-are-8px-squares, and no radius literal
 * escaping the scale anywhere under lib/components/ui/.
 */

const read = (path: string): string => readFileSync(resolve(process.cwd(), path), "utf8");
const layout = read("src/routes/layout.css");

/** Line-anchored @theme token read (a value must sit alone on its declaration line). */
function tokenValue(token: string): string | undefined {
  const match = layout.match(new RegExp(`^\\s*${token}:\\s*([^;]+);`, "m"));
  return match?.[1]?.trim();
}

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

describe("plan 03 radius scale", () => {
  it("tokens match the boards: 8 icon holder, 10 block, 10 collapsed lg, 14 panel, 999 pill, 16 highlight", () => {
    expect(tokenValue("--radius-sm")).toBe("8px");
    expect(tokenValue("--radius-md")).toBe("10px");
    expect(tokenValue("--radius-lg")).toBe("10px");
    expect(tokenValue("--radius-xl")).toBe("14px");
    expect(tokenValue("--radius-pill")).toBe("999px");
    expect(tokenValue("--radius-highlight")).toBe("16px");
  });

  it("shadcn --radius alias tracks the block radius", () => {
    expect(tokenValue("--radius")).toBe("10px");
  });
});

describe("plan 03 no radius literal escapes the scale under ui/", () => {
  const svelteFiles = walk("src/lib/components/ui", ".svelte");

  it("found the ui/ tree (sanity — not vacuous)", () => {
    expect(svelteFiles.length).toBeGreaterThan(20);
  });

  for (const file of svelteFiles) {
    it(`${file} has no rounded-[Npx] literal`, () => {
      expect(read(file).match(/rounded-\[\d+(?:\.\d+)?px\]/), file).toBeNull();
    });
  }
});

describe("plan 03 controls are pills, icon holders are squares", () => {
  const controls: [string, string][] = [
    ["button-variants.ts", read("src/lib/components/ui/button/button-variants.ts")],
    ["icon-button-variants.ts", read("src/lib/components/ui/icon-button/icon-button-variants.ts")],
    ["input.svelte", read("src/lib/components/ui/input/input.svelte")],
    ["tabs-trigger.svelte", read("src/lib/components/ui/tabs/tabs-trigger.svelte")],
    ["Chip.svelte", read("src/lib/components/chip/Chip.svelte")],
  ];

  for (const [name, source] of controls) {
    it(`${name} carries rounded-pill`, () => {
      expect(source).toContain("rounded-pill");
    });
  }

  it("button/icon-button variant tables never use a block radius", () => {
    for (const [, source] of controls.slice(0, 2)) {
      expect(source).not.toContain("rounded-md");
      expect(source).not.toContain("rounded-lg");
    }
  });

  it("MetricCard icon holder is the 8px square, fill/fg resolve via --hue-N/--on-hue-N", () => {
    const card = read("src/lib/components/card/MetricCard.svelte");
    expect(card).toContain("rounded-sm");
    expect(card.match(/var\(--hue-[1-4]\)/g)).toHaveLength(4);
    expect(card.match(/var\(--on-hue-[1-4]\)/g)).toHaveLength(4);
    expect(card.match(/#[0-9a-fA-F]{3,8}\b/)).toBeNull();
  });
});
