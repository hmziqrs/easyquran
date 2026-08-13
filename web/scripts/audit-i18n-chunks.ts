// Per-page localization budget audit.
//
// Walks every prerendered page, resolves the client chunks it preloads, counts the compiled
// Paraglide message bodies inside them, and compares the result against web/i18n-budgets.json.
//
// The number that matters is "how many messages does THIS page download". Paraglide compiles one
// module per message, but a module imported by more than one route is hoisted into a shared chunk,
// so a page can silently pay for copy it never renders. This audit is what keeps that honest.
//
//   node scripts/audit-i18n-chunks.ts            # verify (fails on regression)
//   node scripts/audit-i18n-chunks.ts --update   # rewrite budgets from the current build
//   node scripts/audit-i18n-chunks.ts --report   # print every surface, never fail

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, relative, resolve } from "node:path";

const WEB_ROOT = resolve(import.meta.dirname, "..");
const CLIENT_DIR = join(WEB_ROOT, "build/client");
const PRERENDERED_DIR = join(WEB_ROOT, "build/prerendered");
const BUDGETS_PATH = join(WEB_ROOT, "i18n-budgets.json");

const ASSET_PATTERN = /_app\/immutable\/[a-z]+\/[A-Za-z0-9_.$-]+\.js/g;

// A compiled Paraglide message resolves its locale before returning a variant. Both the readable
// and the minified shape of that check are counted; the minified one is what ships.
const MESSAGE_PATTERNS = [
  /options\.locale \?\? getLocale\(\)/g,
  /\.locale\?\?[A-Za-z_$]+\(\)\)===/g,
];

// Message counts are deterministic, so they are budgeted exactly. Gzipped sizes move by a byte or
// two between builds as chunk hashes and minified identifiers shift, so `--update` leaves this much
// headroom. It is far below the size of a single message, so a real regression still fails.
const GZIP_HEADROOM = 0.02;

export interface Surface {
  id: string;
  match: string[];
  maxMessages: number;
  maxGzip: number;
}

interface Budgets {
  note: string;
  surfaces: Surface[];
}

interface PageMeasurement {
  page: string;
  messages: number;
  gzip: number;
  raw: number;
  chunks: string[];
}

const chunkCache = new Map<string, { messages: number; gzip: number; raw: number }>();

function measureChunk(assetPath: string) {
  const cached = chunkCache.get(assetPath);
  if (cached) return cached;

  let measurement = { messages: 0, gzip: 0, raw: 0 };
  try {
    const bytes = readFileSync(join(CLIENT_DIR, assetPath));
    const text = bytes.toString("utf8");
    const messages = MESSAGE_PATTERNS.reduce(
      (total, pattern) => total + (text.match(pattern)?.length ?? 0),
      0,
    );
    measurement =
      messages > 0 ? { messages, gzip: gzipSync(bytes).length, raw: bytes.length } : measurement;
  } catch {
    // A referenced asset that is absent from build/client is not a localization concern.
  }

  chunkCache.set(assetPath, measurement);
  return measurement;
}

function collectHtml(dir: string, found: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) collectHtml(path, found);
    else if (entry.name.endsWith(".html")) found.push(path);
  }
  return found;
}

function measurePages(): PageMeasurement[] {
  return collectHtml(PRERENDERED_DIR).map((absolute) => {
    const html = readFileSync(absolute, "utf8");
    const assets = [...new Set(html.match(ASSET_PATTERN) ?? [])];
    const measured = assets
      .map((asset) => ({ asset, ...measureChunk(asset) }))
      .filter((chunk) => chunk.messages > 0);

    return {
      page: relative(PRERENDERED_DIR, absolute).replaceAll("\\", "/"),
      messages: measured.reduce((total, chunk) => total + chunk.messages, 0),
      gzip: measured.reduce((total, chunk) => total + chunk.gzip, 0),
      raw: measured.reduce((total, chunk) => total + chunk.raw, 0),
      chunks: measured.map((chunk) => chunk.asset.split("/").at(-1) as string),
    };
  });
}

/** Supports a literal page path and a single trailing `**` prefix match. */
export function matchesSurface(page: string, patterns: string[]): boolean {
  return patterns.some((pattern) =>
    pattern.endsWith("**") ? page.startsWith(pattern.slice(0, -2)) : page === pattern,
  );
}

export function worstPerSurface(pages: PageMeasurement[], surfaces: Surface[]) {
  return surfaces.map((surface) => {
    const matched = pages.filter((page) => matchesSurface(page.page, surface.match));
    const worst = matched.reduce<PageMeasurement | undefined>(
      (worstSoFar, page) =>
        !worstSoFar || page.messages > worstSoFar.messages ? page : worstSoFar,
      undefined,
    );
    return { surface, matched: matched.length, worst };
  });
}

const budgets = JSON.parse(readFileSync(BUDGETS_PATH, "utf8")) as Budgets;
const pages = measurePages();

if (pages.length === 0) {
  throw new Error("[i18n-budget] no prerendered pages found — run pnpm build first");
}

const results = worstPerSurface(pages, budgets.surfaces);
function cliMode(): "update" | "report" | "verify" {
  if (process.argv.includes("--update")) return "update";
  if (process.argv.includes("--report")) return "report";
  return "verify";
}

const mode = cliMode();

const pad = (value: string | number, width: number) => String(value).padEnd(width);
console.log(`[i18n-budget] ${pad("surface", 24)} ${pad("pages", 7)} ${pad("messages", 20)} gzip`);

const failures: string[] = [];
for (const { surface, matched, worst } of results) {
  if (!worst) {
    failures.push(`${surface.id}: no prerendered page matched ${surface.match.join(", ")}`);
    continue;
  }

  const messageLabel = `${worst.messages} / ${surface.maxMessages}`;
  const gzipLabel = `${(worst.gzip / 1024).toFixed(1)}k / ${(surface.maxGzip / 1024).toFixed(1)}k`;
  const over = worst.messages > surface.maxMessages || worst.gzip > surface.maxGzip;
  console.log(
    `${over ? "  FAIL" : "  ok  "} ${pad(surface.id, 24)} ${pad(matched, 7)} ${pad(messageLabel, 20)} ${gzipLabel}   ${worst.page}`,
  );

  if (over && mode === "verify") {
    failures.push(
      `${surface.id} regressed: ${worst.page} ships ${worst.messages} messages / ${worst.gzip} B gzip ` +
        `(budget ${surface.maxMessages} / ${surface.maxGzip}). Split the copy it does not render, ` +
        `or justify the raise in the PR.`,
    );
  }

  if (mode === "update") {
    surface.maxMessages = worst.messages;
    surface.maxGzip = Math.ceil(worst.gzip * (1 + GZIP_HEADROOM));
  }
}

if (mode === "update") {
  writeFileSync(BUDGETS_PATH, `${JSON.stringify(budgets, null, 2)}\n`);
  console.log(`[i18n-budget] budgets rewritten from current build`);
} else if (failures.length > 0 && mode === "verify") {
  console.error(`\n[i18n-budget] ${failures.length} failure(s):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
} else {
  const total = results.reduce((sum, entry) => sum + (entry.worst?.messages ?? 0), 0);
  console.log(
    `[i18n-budget] ${results.length} surfaces within budget (${total} worst-case messages)`,
  );
}
