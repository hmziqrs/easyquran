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
