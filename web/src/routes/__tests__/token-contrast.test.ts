import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { evaluateContrast } from "../../../scripts/visual/contrast.ts";

/**
 * The contrast gate (docs/plan/07-visual-verification.md). Computes WCAG contrast for
 * every token pair in every one of the eight `[data-palette][data-mode]` blocks of
 * layout.css and fails below threshold — a reviewer cannot eyeball 4.5:1, so this gate
 * converts the migration's main risk into a build failure. The pure core lives in
 * scripts/visual/contrast.ts and is shared with its CLI (`node scripts/visual/contrast.ts`).
 */
// Read via process.cwd() (vitest runs from web/) — `new URL(literal, import.meta.url)` gets
// rewritten by Vite's asset transform for .css targets and resolves to a non-file URL.
const css = readFileSync(resolve(process.cwd(), "src/routes/layout.css"), "utf8");
const report = evaluateContrast(css);

describe("token contrast gate", () => {
  it("finds all eight palette blocks", () => {
    expect(report.blockCount).toBe(8);
  });

  it("asserts the contract pairs everywhere — only additive hue pairs may be skipped", () => {
    const skipped = report.rows.filter((r) => r.status === "skipped");
    for (const row of skipped) {
      expect(row.fg, `${row.palette} ${row.mode}: unexpected skip`).toMatch(/^--on-hue-[1-4]$/);
    }
    // 8 asserted pairs per block (reading, secondary, muted ×2, primary, translation,
    // border floor); the 4 hue pairs join automatically once plan 01 defines them.
    const asserted = report.rows.filter((r) => r.status !== "skipped");
    expect(asserted.length).toBeGreaterThanOrEqual(64);
  });

  it("computes a ratio for every asserted pair (no unparseable token values)", () => {
    const ratioless = report.rows.filter((r) => r.status !== "skipped" && r.ratio === null);
    expect(ratioless.map((r) => r.message)).toEqual([]);
  });

  it("meets the threshold for every pair in every palette and mode", () => {
    // One assertion over the collected failures so the message lists every offender at once.
    expect(report.failures.map((r) => r.message)).toEqual([]);
  });

  it("keeps reading surfaces at AAA (7:1) — this is a reading product", () => {
    const reading = report.rows.filter(
      (r) =>
        r.ratio !== null &&
        (r.fg === "--foreground" || r.fg === "--quran-foreground") &&
        r.bg !== "--primary",
    );
    expect(reading.length).toBe(16); // 2 pairs × 8 blocks
    for (const row of reading) {
      expect(row.ratio, row.message).toBeGreaterThanOrEqual(7);
    }
  });
});

describe("hue-pair auto-activation (judge round 1: no per-block silent skip)", () => {
  it("fails a block that misses a hue pair defined in any other block", () => {
    // The 7/8-blocks trap: plan 01 defines the hue set everywhere except one block — the
    // gate must go red on that block, not bless it as a skip.
    const synthetic = [
      '[data-palette="sacred"][data-mode="light"] {',
      "  --hue-1: #000000;",
      "  --on-hue-1: #ffffff;",
      "}",
      '[data-palette="sacred"][data-mode="dark"] {',
      "}",
    ].join("\n");
    const result = evaluateContrast(synthetic);
    expect(result.blockCount).toBe(2);
    const hue1 = result.rows.filter((r) => r.fg === "--on-hue-1" && r.bg === "--hue-1");
    expect(hue1.filter((r) => r.status === "skipped")).toEqual([]);
    const failedBlocks = hue1.filter((r) => r.status === "fail").map((r) => r.mode);
    expect(failedBlocks).toEqual(["dark"]);
    const light = hue1.find((r) => r.mode === "light");
    expect(light?.status).toBe("pass");
    expect(light?.ratio).toBeCloseTo(21, 5); // #ffffff on #000000
  });

  it("still skips cleanly while a hue pair is defined nowhere", () => {
    const synthetic = [
      '[data-palette="ink"][data-mode="light"] {',
      "}",
      '[data-palette="ink"][data-mode="dark"] {',
      "}",
    ].join("\n");
    const result = evaluateContrast(synthetic);
    const skipped = result.rows.filter((r) => r.status === "skipped");
    // 4 hue pairs × 2 blocks; every skip is a hue pair, never a contract pair.
    expect(skipped.length).toBe(8);
    for (const row of skipped) {
      expect(row.fg).toMatch(/^--on-hue-[1-4]$/);
    }
  });
});
