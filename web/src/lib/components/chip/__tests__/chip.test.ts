import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

/**
 * Machine guard for the §19 badge / §33 filter-chip contract (docs/design-system.md):
 * 24px pill, first-match tones on semantic tokens, active state never color-only
 * (aria-pressed on the interactive variant).
 */
const chip = readFileSync(resolve(process.cwd(), "src/lib/components/chip/Chip.svelte"), "utf8");

describe("chip §19/§33 + plan 03 geometry/state matrix", () => {
  it("badge sizing: 24px height, 8px inline padding, 11/16 medium, pill radius", () => {
    expect(chip).toContain("h-6");
    expect(chip).toContain("rounded-pill");
    expect(chip).toContain("text-[11px]");
    expect(chip).toContain("font-medium");
  });

  it("is a pill control — never a block radius (plan 03 geometry)", () => {
    expect(chip).not.toContain("rounded-md");
    expect(chip).not.toContain("rounded-lg");
  });

  it("active tone is primary/primary-foreground; inactive is surface/border (§33)", () => {
    expect(chip).toContain("bg-primary text-primary-foreground");
    expect(chip).toContain("border-border bg-surface text-foreground-secondary");
  });

  it("accent tone maps to the editorial gold pair (§19 Makki)", () => {
    expect(chip).toContain("bg-gold-soft text-gold-strong");
  });

  it("interactive variant exposes state via aria-pressed, not color alone (§51)", () => {
    expect(chip).toContain("aria-pressed={active}");
    expect(chip).toContain('type="button"');
  });

  it("state matrix: inactive hover is --surface-hover only (no border-strong — that is the row pattern)", () => {
    expect(chip).toContain("hover:bg-surface-hover");
    expect(chip).not.toContain("hover:border-border-strong");
  });

  it("the marker dot is the only genuinely-round element (status dot, not an icon holder)", () => {
    expect(chip.match(/rounded-full/g)).toHaveLength(2);
    expect(chip).toContain("size-1.5 rounded-full bg-gold");
  });

  it("contains no hard-coded palette color (§61)", () => {
    expect(chip.match(/#[0-9a-fA-F]{3,8}\b/)).toBeNull();
  });
});
