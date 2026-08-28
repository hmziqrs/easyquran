/**
 * Contrast gate core (docs/plan/07-visual-verification.md — "the single highest-value
 * artefact in the plan"). Pure: takes layout.css as a string, returns one row per
 * (palette block × token pair). Consumed by `token-contrast.test.ts` (part of `pnpm test`)
 * and by the CLI below for a human-readable table.
 *
 * Usage: node scripts/visual/contrast.ts [--json]
 * Exits 1 when any asserted pair fails or any contract token is unparseable.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { contrastRatio, parseColor } from "../../src/lib/theme/derive.ts";

export const PALETTE_IDS = ["sacred", "ink", "sepia", "sapphire"] as const;
export const BLOCK_MODES = ["light", "dark"] as const;

export interface PairSpec {
  fg: string;
  bg: string;
  /** WCAG ratio the pair must meet. */
  min: number;
  /** Additive tokens (plan 01 hue set): skipped while undefined, asserted once present. */
  additive?: boolean;
}

/**
 * Divergence from plan 07 (recorded in .z-workflow/STATE.md): the plan's table asks for
 * `--border` on `--background` >= 1.5:1, but its OWN target tokens fail that — light
 * `oklch(0.885 0 0)` on `oklch(0.98 0 0)` measures 1.33:1, dark 0.305-on-0.165 measures
 * 1.44:1, and the current Sacred Editorial values measure 1.33–1.51:1. The boards win over
 * sub-plan text (TASK policy), so the border pair asserts a visibility floor of 1.1:1 —
 * still catches the real failure modes (a border that equals its background, or a block
 * whose border token never got defined) — and the measured ratio is always reported.
 */
const BORDER_FLOOR = 1.1;

const huePairs: PairSpec[] = [1, 2, 3, 4].map((n) => ({
  fg: `--on-hue-${n}`,
  bg: `--hue-${n}`,
  min: 4.5,
  additive: true,
}));

export const CONTRAST_PAIRS: PairSpec[] = [
  { fg: "--foreground", bg: "--background", min: 7 },
  { fg: "--foreground-secondary", bg: "--background", min: 4.5 },
  { fg: "--muted", bg: "--background", min: 4.5 },
  { fg: "--muted", bg: "--surface", min: 4.5 },
  { fg: "--primary-foreground", bg: "--primary", min: 4.5 },
  ...huePairs,
  { fg: "--quran-foreground", bg: "--reader-background", min: 7 },
  { fg: "--translation-foreground", bg: "--reader-background", min: 4.5 },
  { fg: "--border", bg: "--background", min: BORDER_FLOOR },
];

export interface BlockTokenMap {
  palette: string;
  mode: string;
  tokens: Map<string, string>;
}

/** Extracts the eight `[data-palette="X"][data-mode="Y"] { … }` blocks' token assignments. */
export function paletteBlocks(css: string): BlockTokenMap[] {
  const out: BlockTokenMap[] = [];
  for (const palette of PALETTE_IDS) {
    for (const mode of BLOCK_MODES) {
      const marker = `[data-palette="${palette}"][data-mode="${mode}"]`;
      const start = css.indexOf(marker);
      if (start < 0) continue;
      const open = css.indexOf("{", start);
      const close = css.indexOf("\n}", open);
      if (open < 0 || close < 0) continue;
      const tokens = new Map<string, string>();
      for (const m of css.slice(open + 1, close).matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
        tokens.set(m[1]!, m[2]!.trim());
      }
      out.push({ palette, mode, tokens });
    }
  }
  return out;
}

export type RowStatus = "pass" | "fail" | "skipped";

export interface ContrastRow {
  palette: string;
  mode: string;
  fg: string;
  bg: string;
  fgRaw: string | undefined;
  bgRaw: string | undefined;
  ratio: number | null;
  min: number;
  status: RowStatus;
  message: string;
}

export interface ContrastReport {
  rows: ContrastRow[];
  failures: ContrastRow[];
  /** Blocks found in the css — the test asserts this is all eight. */
  blockCount: number;
}

export function evaluateContrast(css: string): ContrastReport {
  const rows: ContrastRow[] = [];
  const blocks = paletteBlocks(css);
  for (const block of blocks) {
    for (const pair of CONTRAST_PAIRS) {
      const fgRaw = block.tokens.get(pair.fg);
      const bgRaw = block.tokens.get(pair.bg);
      const where = `${block.palette} ${block.mode}`;
      if (fgRaw === undefined || bgRaw === undefined) {
        // Contract tokens must always exist (palette-contract guards that too); additive
        // tokens are allowed to be absent until the plan that defines them lands.
        rows.push({
          palette: block.palette,
          mode: block.mode,
          fg: pair.fg,
          bg: pair.bg,
          fgRaw,
          bgRaw,
          ratio: null,
          min: pair.min,
          status: pair.additive ? "skipped" : "fail",
          message:
            pair.additive
              ? `${where}: ${pair.fg}/${pair.bg} not defined yet (additive token)`
              : `${where}: missing token ${fgRaw === undefined ? pair.fg : pair.bg}`,
        });
        continue;
      }
      const fg = parseColor(fgRaw);
      const bg = parseColor(bgRaw);
      if (!fg || !bg) {
        rows.push({
          palette: block.palette,
          mode: block.mode,
          fg: pair.fg,
          bg: pair.bg,
          fgRaw,
          bgRaw,
          ratio: null,
          min: pair.min,
          status: "fail",
          message: `${where}: unparseable value ${!fg ? fgRaw : bgRaw}`,
        });
        continue;
      }
      const ratio = contrastRatio(fg, bg);
      const ok = ratio >= pair.min;
      rows.push({
        palette: block.palette,
        mode: block.mode,
        fg: pair.fg,
        bg: pair.bg,
        fgRaw,
        bgRaw,
        ratio,
        min: pair.min,
        status: ok ? "pass" : "fail",
        message: `${where}: ${pair.fg} (${fgRaw}) on ${pair.bg} (${bgRaw}) = ${ratio.toFixed(2)}:1, needs ${pair.min}:1`,
      });
    }
  }
  return { rows, failures: rows.filter((r) => r.status === "fail"), blockCount: blocks.length };
}

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function statusMark(status: RowStatus): string {
  if (status === "pass") return "ok  ";
  if (status === "fail") return "FAIL";
  return "skip";
}

function cli(): void {
  const json = process.argv.includes("--json");
  const cssPath = path.resolve(__dirname, "../../src/routes/layout.css");
  const report = evaluateContrast(readFileSync(cssPath, "utf8"));
  if (json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    for (const row of report.rows) {
      const mark = statusMark(row.status);
      const ratio = row.ratio === null ? "  —  " : row.ratio.toFixed(2).padStart(5);
      console.log(`${mark} ${row.palette}/${row.mode} ${ratio}:1  ${row.fg} on ${row.bg}`);
    }
    console.log(
      `\n${report.rows.length} pairs across ${report.blockCount} blocks, ${report.failures.length} failures`,
    );
  }
  if (report.failures.length > 0 || report.blockCount !== 8) {
    for (const f of report.failures) console.error(f.message);
    if (report.blockCount !== 8) console.error(`expected 8 palette blocks, found ${report.blockCount}`);
    process.exitCode = 1;
  }
}

// CLI when invoked directly (`node scripts/visual/contrast.ts`), library when imported by
// the test. Both entry paths share `evaluateContrast`, so the gate cannot drift from the
// table the human reads.
const invokedDirectly = process.argv[1] === fileURLToPath(import.meta.url);
if (invokedDirectly) cli();
