/**
 * quran.com ↔ local DB parity audit (report tool, never a gate).
 *
 * Usage: bun run quran:audit [--out <path>]
 *
 * Runner: tsx (package.json "quran:audit"). node:sqlite is available under tsx on this
 * toolchain — `node -e "import('node:sqlite').then(m => console.log(typeof m.DatabaseSync))"`
 * prints "function" (Node 24) — so no bun:sqlite fallback is needed.
 *
 * Reads the committed mirror (web/static/qurancom/scripts/*.json) and the provisioned,
 * gitignored DBs (db/quran/arabic/*.sqlite, read-only, immutable). Writes a markdown report
 * to .zflow/evidence/qurancom-parity-report.md by default (gitignored session material).
 * Identity is id + byte size; this script never hashes and never touches db/.
 * Exit 0 always — except a missing mirror exits 1 with the bun run quran:mirror instruction.
 */

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";

import { QURANCOM_ASSETS } from "./qurancom-assets.ts";
import {
  classifyMismatch,
  decodeMirrorVerses,
  diffSummary,
  formatCodePoint,
  mirrorBareText,
  ourBareText,
  QURANCOM_PAIRS,
  type ParityBucket,
  type QurancomOurDb,
  type QurancomPair,
} from "../../web/src/routes/design/qurancom/compare.ts";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const MIRROR_SCRIPTS = path.join(REPO_ROOT, "web", "static", "qurancom", "scripts");
const DB_DIR = path.join(REPO_ROOT, "db", "quran", "arabic");
const DEFAULT_OUT = path.join(REPO_ROOT, ".zflow", "evidence", "qurancom-parity-report.md");
const TAIL_LENGTH = 6;
const MEDALLION_VERSES = ["13:26", "13:27", "13:28", "13:29", "1:1", "114:6"];
const QUIRK_VERSE = "22:77";

interface BucketTally {
  readonly bucket: ParityBucket;
  readonly count: number;
  readonly samples: readonly string[];
  readonly excerpt: string;
}

interface PairAudit {
  readonly pair: QurancomPair;
  readonly our: QurancomOurDb;
  readonly present: boolean;
  readonly note: string | null;
  readonly dbVerses: number;
  readonly compared: number;
  readonly exact: number;
  readonly mismatch: number;
  readonly buckets: readonly BucketTally[];
}

interface TailRow {
  readonly key: string;
  readonly mirrorTail: string | null;
  readonly oursTail: string | null;
}

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function log(message: string): void {
  process.stdout.write(`${message}\n`);
}

function mirrorScriptPath(file: string): string {
  return path.join(MIRROR_SCRIPTS, file);
}

function registrySize(file: string): number | null {
  const asset = QURANCOM_ASSETS.find((entry) => entry.targetPath === `scripts/${file}`);
  if (asset === undefined || asset.sizeBytes === 0) return null;
  return asset.sizeBytes;
}

function openReadOnly(absolute: string): DatabaseSync {
  const url = pathToFileURL(absolute);
  url.search = "?mode=ro&immutable=1";
  return new DatabaseSync(url, { readOnly: true });
}

function windowAround(text: string, index: number): string {
  const codePoints = Array.from(text);
  return codePoints.slice(Math.max(0, index - 10), index + 16).join("");
}

function auditPairDb(
  pair: QurancomPair,
  our: QurancomOurDb,
  mirror: ReadonlyMap<string, string>,
): PairAudit {
  const base = {
    pair,
    our,
    present: false,
    note: null,
    dbVerses: 0,
    compared: 0,
    exact: 0,
    mismatch: 0,
  };
  const dbPath = path.join(DB_DIR, our.dbFile);
  if (!existsSync(dbPath)) {
    return { ...base, note: "DB not provisioned — run just quran-fetch", buckets: [] };
  }
  let database: DatabaseSync;
  try {
    database = openReadOnly(dbPath);
  } catch (error) {
    return { ...base, note: `open failed: ${describeError(error)}`, buckets: [] };
  }
  try {
    const rows = database.prepare('SELECT sura, aya, text FROM quran_text ORDER BY "index"').all();
    const buckets = new Map<ParityBucket, { count: number; samples: string[]; excerpt: string }>();
    let compared = 0;
    let exact = 0;
    let mismatch = 0;
    for (const row of rows) {
      const key = `${Number(row.sura)}:${Number(row.aya)}`;
      const mirrorRaw = mirror.get(key);
      if (mirrorRaw === undefined) continue;
      compared += 1;
      const oursRaw = String(row.text);
      const mirrorBare = mirrorBareText(pair, mirrorRaw);
      const oursBare = ourBareText(pair, oursRaw);
      const diff = diffSummary(mirrorBare, oursBare);
      if (diff.equal) {
        exact += 1;
        continue;
      }
      mismatch += 1;
      const bucket = classifyMismatch({
        pair,
        mirrorRaw,
        oursRaw,
        mirrorBare,
        oursBare,
      });
      const index = diff.firstIndex ?? 0;
      const existing = buckets.get(bucket);
      if (existing === undefined) {
        const excerpt = `${key}: ${JSON.stringify(windowAround(mirrorBare, index))} ≠ ${JSON.stringify(windowAround(oursBare, index))}`;
        buckets.set(bucket, { count: 1, samples: [key], excerpt });
        continue;
      }
      existing.count += 1;
      if (existing.samples.length < 5) existing.samples.push(key);
    }
    const tallies = [...buckets.entries()]
      .map(([bucket, tally]) => ({ bucket, ...tally }))
      .sort((a, b) => b.count - a.count);
    return {
      ...base,
      present: true,
      dbVerses: rows.length,
      compared,
      exact,
      mismatch,
      buckets: tallies,
    };
  } finally {
    database.close();
  }
}

function tailOf(text: string): string {
  return Array.from(text)
    .slice(-TAIL_LENGTH)
    .map((value) => formatCodePoint(value.codePointAt(0) ?? null))
    .join(" ");
}

function medallionRows(): readonly TailRow[] {
  const pair = QURANCOM_PAIRS.find((entry) => entry.slug === "indopak_nastaleeq");
  if (pair === undefined) return [];
  const mirror = decodeMirrorVerses(
    readFileSync(mirrorScriptPath(pair.mirrorFile), "utf8"),
    pair.mirrorField,
  );
  const dbPath = path.join(DB_DIR, pair.ours[0]?.dbFile ?? "");
  const database = existsSync(dbPath) ? openReadOnly(dbPath) : null;
  try {
    return [...MEDALLION_VERSES, QUIRK_VERSE].map((key) => {
      const mirrorRaw = mirror.get(key) ?? null;
      let oursRaw: string | null = null;
      if (database !== null) {
        const [sura, aya] = key.split(":").map((part) => Number(part));
        const row = database
          .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
          .get(sura ?? 0, aya ?? 0);
        if (row) oursRaw = String(row.text);
      }
      return {
        key,
        mirrorTail: mirrorRaw === null ? null : tailOf(mirrorRaw),
        oursTail: oursRaw === null ? null : tailOf(oursRaw),
      };
    });
  } finally {
    database?.close();
  }
}

function dominantBucket(audit: PairAudit): string {
  const top = audit.buckets[0];
  if (top === undefined) return "—";
  return `${top.bucket} (${top.count})`;
}

function mirrorFileRows(): readonly { file: string; bytes: number; registry: number | null }[] {
  return QURANCOM_PAIRS.map((pair) => {
    const file = pair.mirrorFile;
    return {
      file,
      bytes: statSync(mirrorScriptPath(file)).size,
      registry: registrySize(file),
    };
  });
}

function registryState(row: { bytes: number; registry: number | null }): string {
  if (row.registry === null) return "not in registry";
  if (row.registry === row.bytes) return "ok";
  return `drift — registry ${row.registry}, rebake with bun run quran:mirror --record`;
}

function buildReport(
  generatedAt: string,
  files: readonly { file: string; bytes: number; registry: number | null }[],
  audits: readonly PairAudit[],
  tails: readonly TailRow[],
): string {
  const lines: string[] = [];
  lines.push("# quran.com parity audit", "");
  lines.push(`Generated: ${generatedAt}`, "");
  lines.push(
    "Basis: stored text per pair; `uthmani_tajweed` compares markup-stripped bare text because",
    "the markup formats differ by design (ours `[h:…[` runs, theirs `<tajweed …>` HTML).",
    "Identity is id + byte size; no hashes anywhere.",
    "",
  );
  lines.push("## Mirror files", "");
  lines.push("| file | bytes | registry | state |", "| --- | ---: | ---: | --- |");
  for (const row of files) {
    lines.push(
      `| scripts/${row.file} | ${row.bytes} | ${row.registry ?? "—"} | ${registryState(row)} |`,
    );
  }
  lines.push("", "## Summary", "");
  lines.push(
    "| pair | our db | verses | exact | mismatch | dominant bucket |",
    "| --- | --- | ---: | ---: | ---: | --- |",
  );
  for (const audit of audits) {
    if (!audit.present) {
      lines.push(`| ${audit.pair.slug} | ${audit.our.label} | — | — | — | ${audit.note} |`);
      continue;
    }
    lines.push(
      `| ${audit.pair.slug} | ${audit.our.label} | ${audit.compared} | ${audit.exact} | ${audit.mismatch} | ${dominantBucket(audit)} |`,
    );
  }
  const notProvisioned = audits.filter((audit) => !audit.present);
  if (notProvisioned.length > 0) {
    lines.push(
      "",
      "Not provisioned locally: " + notProvisioned.map((audit) => audit.our.label).join(", ") + ".",
    );
  }
  lines.push("", "## Buckets", "");
  for (const audit of audits) {
    if (!audit.present) continue;
    lines.push(`### ${audit.pair.slug} × ${audit.our.label} — ${audit.mismatch} mismatches`, "");
    if (audit.buckets.length === 0) {
      lines.push("- no mismatches", "");
      continue;
    }
    for (const tally of audit.buckets) {
      lines.push(`- **${tally.bucket}** (${tally.count}): samples ${tally.samples.join(", ")}`);
      lines.push(`  \`${tally.excerpt}\``);
    }
    lines.push("");
  }
  lines.push("## Nastaleeq medallion tails", "");
  lines.push(
    `Last ${TAIL_LENGTH} codepoints per side for the known artifact verses (medallion grammar visibility).`,
    "",
  );
  lines.push("| verse | mirror tail | ours tail | state |", "| --- | --- | --- | --- |");
  for (const row of tails) {
    const mirrorTail = row.mirrorTail ?? "—";
    const oursTail = row.oursTail ?? "—";
    let state = "match";
    if (row.mirrorTail === null || row.oursTail === null) state = "one side missing";
    else if (row.mirrorTail !== row.oursTail) state = "differ";
    lines.push(`| ${row.key} | ${mirrorTail} | ${oursTail} | ${state} |`);
  }
  const quirk = tails.find((row) => row.key === QUIRK_VERSE);
  if (quirk !== undefined) {
    const note =
      quirk.mirrorTail !== null && quirk.mirrorTail === quirk.oursTail
        ? "expected upstream quirk (mark-only ending), ours matches"
        : "expected upstream quirk (mark-only ending), tails differ — inspect";
    lines.push("", `${QUIRK_VERSE}: ${note}.`, "");
  }
  return `${lines.join("\n")}\n`;
}

function parseOut(args: readonly string[]): string {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === undefined) continue;
    if (arg === "--out") {
      const value = args[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error("--out requires a path");
      }
      return path.resolve(process.cwd(), value);
    }
    if (arg.startsWith("--out=")) return path.resolve(process.cwd(), arg.slice("--out=".length));
    throw new Error(`unknown option: ${arg}`);
  }
  return DEFAULT_OUT;
}

function runAudit(): readonly PairAudit[] {
  const audits: PairAudit[] = [];
  for (const pair of QURANCOM_PAIRS) {
    const mirror = decodeMirrorVerses(
      readFileSync(mirrorScriptPath(pair.mirrorFile), "utf8"),
      pair.mirrorField,
    );
    for (const our of pair.ours) audits.push(auditPairDb(pair, our, mirror));
  }
  return audits;
}

function main(): void {
  const out = parseOut(process.argv.slice(2));
  const missing = QURANCOM_PAIRS.map((pair) => pair.mirrorFile).filter(
    (file) => !existsSync(mirrorScriptPath(file)),
  );
  if (missing.length > 0) {
    process.stderr.write(
      `quran.com mirror incomplete (${missing.join(", ")}); run bun run quran:mirror\n`,
    );
    process.exitCode = 1;
    return;
  }
  const audits = runAudit();
  const report = buildReport(new Date().toISOString(), mirrorFileRows(), audits, medallionRows());
  mkdirSync(path.dirname(out), { recursive: true });
  writeFileSync(out, report);
  log(`report: ${out}`);
  for (const audit of audits) {
    const state = audit.present ? `${audit.exact} exact / ${audit.mismatch} mismatch` : audit.note;
    log(`${audit.pair.slug} × ${audit.our.label}: ${state}`);
  }
}

const isDirectRun =
  process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectRun) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`quran.com parity audit failed: ${describeError(error)}\n`);
    process.exitCode = 1;
  }
}
