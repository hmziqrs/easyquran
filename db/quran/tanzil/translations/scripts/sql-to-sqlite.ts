/**
 * sql-to-sqlite.ts — convert the mirrored Tanzil MySQL dumps into SQLite.
 *
 *   tsx scripts/sql-to-sqlite.ts            # build sqlite/<id>.sqlite for all
 *   tsx scripts/sql-to-sqlite.ts en.sahih   # ...only the given ids
 *
 * Each output is a standalone `sqlite/<id>.sqlite` whose single table mirrors
 * the Arabic Tanzil convention and the canonical schema in
 * docs/quran-system.md:
 *
 *     CREATE TABLE quran_text ("index" INTEGER PRIMARY KEY, sura, aya, text);
 *     CREATE INDEX idx_quran_text_sura_aya ON quran_text (sura, aya);
 *
 * The hard part is fidelity. These dumps are phpMyAdmin MySQL exports whose
 * string-literal escaping is **not uniform**: ~68 files escape with backslashes
 * (`\'`, `\"`, `\n` …) while the rest use the SQL-standard doubled quote (`''`),
 * where `\` is a literal character. SQLite does NOT interpret `\'` (it stores a
 * literal backslash), so piping the dumps through sqlite3 corrupts verse text.
 *
 * We parse each tuple with a real MySQL-string scanner, decode to the true
 * string, and bind it as a parameter — letting SQLite do its own, correct
 * quoting. Escaping mode is detected **per file**: both modes are parsed, and
 * if only one is self-consistent (6236 contiguous rows) that wins — e.g. a
 * standard-mode file whose stray `\` before a quote makes backslash mode merge
 * rows. If both parse cleanly (the backslashes present don't affect string
 * termination), the file's apostrophe convention decides: `''` without `\'`
 * ⇒ standard mode (`\` literal), `\'` ⇒ backslash mode, neither ⇒ backslash.
 *
 * Builds run through Node's built-in `node:sqlite` (v24) — zero dependencies,
 * per the repo's stated SQLite principle.
 */

import { DatabaseSync } from "node:sqlite";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync } from "node:fs";
import path from "node:path";

import { ROOT, SQLDIR, SQLITEDIR, MANIFEST, INDEX, log, readJson, writeJson, fileSha256, type IndexFile, type SqliteDigest } from "./lib";

const OUTDIR = SQLITEDIR;

// MySQL backslash escapes inside single-quoted string literals.
// Anything not listed keeps the following character verbatim (MySQL behavior).
function decodeEscape(nx: string): string {
  switch (nx) {
    case "0": return "\0";
    case "b": return "\b";
    case "n": return "\n";
    case "r": return "\r";
    case "t": return "\t";
    case "Z": return "\x1A";
    case "\\": return "\\";
    case "'": return "'";
    case '"': return '"';
    case "`": return "`";
    default: return nx;
  }
}

interface Row {
  index: number;
  sura: number;
  aya: number;
  text: string;
}

/**
 * Parse every `(index, sura, aya, text)` tuple out of a Tanzil MySQL dump.
 *
 * A dump holds one INSERT per sura (114 total), each preceded by a
 * `-- Sura N ()` line comment. We strip full-line comments first (so the `()`
 * in them can't masquerade as a tuple paren), then walk each INSERT statement,
 * reading its column list once and scanning its VALUES tuples with a
 * string-state machine that honours both `\'` and `''` escaping. Strings are
 * consumed to their true closing quote, so commas / parens / semicolons inside
 * verse text never break the scan.
 */
function parseRows(sql: string, backslashMode = true): Row[] {
  const cleaned = sql.replace(/^[ \t]*(--|#).*$/gm, "");
  const rows: Row[] = [];
  const n = cleaned.length;
  const insertRe = /INSERT\s+INTO\b/gi;
  let m: RegExpExecArray | null;

  while ((m = insertRe.exec(cleaned)) !== null) {
    let p = m.index + m[0].length;

    // table name (optionally backticked; tolerate dotted/hyphenated ids)
    const tbl = cleaned.slice(p).match(/^\s*`?[\w.-]+`?\s*/);
    if (tbl) p += tbl[0].length;

    // column list
    let cols = ["index", "sura", "aya", "text"];
    if (cleaned[p] === "(") {
      const close = cleaned.indexOf(")", p);
      if (close < 0) { insertRe.lastIndex = p; continue; }
      cols = cleaned.slice(p + 1, close).split(",").map((s) => s.replace(/`/g, "").trim().toLowerCase());
      p = close + 1;
    }
    const idxIdx = cols.indexOf("index");
    const suraIdx = cols.indexOf("sura");
    const ayaIdx = cols.indexOf("aya");
    const textIdx = cols.indexOf("text");
    if (idxIdx < 0 || suraIdx < 0 || ayaIdx < 0 || textIdx < 0) {
      insertRe.lastIndex = p;
      continue;
    }

    const vals = cleaned.slice(p).match(/^\s*VALUES\b\s*/i);
    if (!vals) { insertRe.lastIndex = p; continue; }
    p += vals[0].length;

    const raw: string[] = new Array(cols.length);

    // scan tuples until the statement's terminating ';'
    while (p < n) {
      // skip inter-tuple separators: commas, whitespace, and the ')'
      // that closed the previous tuple (after comment-stripping the only ')'
      // at this level is a tuple closer)
      let c = cleaned[p]!;
      while (p < n && (c === " " || c === "\t" || c === "\n" || c === "\r" || c === "," || c === ")")) c = cleaned[++p]!;
      if (p >= n || c !== "(") break; // ';' or end → done with this INSERT
      p++; // past '('

      for (let col = 0; col < cols.length; col++) {
        // skip intra-tuple separators
        while (p < n && ((c = cleaned[p]!) === " " || c === "\t" || c === "\n" || c === "\r" || c === ",")) p++;
        if (p >= n) break;

        if (cleaned[p] === "'") {
          p++; // past opening quote
          let buf = "";
          while (p < n) {
            const d = cleaned[p]!;
            if (backslashMode && d === "\\") {
              const nx = cleaned[p + 1];
              buf += nx === undefined ? "" : decodeEscape(nx);
              p += 2;
            } else if (d === "'") {
              if (cleaned[p + 1] === "'") { buf += "'"; p += 2; } // doubled quote
              else { p++; break; } // closing quote
            } else {
              buf += d;
              p++;
            }
          }
          raw[col] = buf;
        } else {
          // unquoted token (integer); read until ',' or ')'
          let j = p;
          while (j < n && cleaned[j] !== "," && cleaned[j] !== ")") j++;
          raw[col] = cleaned.slice(p, j).trim();
          p = j;
        }
      }

      rows.push({
        index: Number.parseInt(raw[idxIdx]!, 10),
        sura: Number.parseInt(raw[suraIdx]!, 10),
        aya: Number.parseInt(raw[ayaIdx]!, 10),
        text: raw[textIdx]!,
      });
    }
    insertRe.lastIndex = p;
  }
  return rows;
}

function isContiguous(rows: Row[]): boolean {
  for (let i = 0; i < rows.length; i++) if (rows[i]!.index !== i + 1) return false;
  return true;
}

interface BuildResult {
  id: string;
  outPath: string;
  rows: number;
  expected: number | undefined;
  ok: boolean;
  integrity: string;
  sizeBytes: number;
  sha256: string;
  mode: "backslash" | "standard";
  notes: string[];
}

// Canonical full-Quran ayah count (Hafs) — matches store.rs VERSE_COUNT. A translation must tile
// exactly this many rows; the runtime's validate_rows rejects anything else, so the build gate
// enforces the same count and a partial dump fails here instead of at every read.
const VERSE_COUNT = 6236;

function buildOne(id: string, expected: number | undefined): BuildResult {
  const sqlPath = path.join(SQLDIR, `${id}.sql`);
  const outPath = path.join(OUTDIR, `${id}.sqlite`);
  const notes: string[] = [];

  if (existsSync(outPath)) rmSync(outPath); // clean rebuild — no leftover schema
  const sql = readFileSync(sqlPath, "utf-8");

  // Detect the dump's escaping mode per file. Parse both modes; if only one is
  // clean (e.g. a standard-mode file whose stray `\` before a quote makes
  // backslash mode merge rows), use it. If BOTH are clean — the backslashes
  // present don't affect string termination — decide by the file's apostrophe
  // convention: `''` without `\'` ⇒ standard SQL mode (`\` literal); `\'` ⇒
  // backslash mode; neither ⇒ backslash (drops stray backslashes in text that
  // has no apostrophe escaping at all).
  const clean = (rs: Row[]): boolean =>
    rs.length > 0 && isContiguous(rs) && (expected === undefined || rs.length === expected);
  const backslash = parseRows(sql, true);
  const standard = parseRows(sql, false);
  let rows = backslash;
  let mode: "backslash" | "standard" = "backslash";
  if (clean(backslash) && clean(standard)) {
    const hasBackslashEsc = sql.includes("\\'");
    const hasDoubled = sql.includes("''");
    if (hasDoubled && !hasBackslashEsc) {
      rows = standard;
      mode = "standard";
    }
  } else if (clean(standard) || standard.length > backslash.length) {
    rows = standard;
    mode = "standard";
  } else {
    rows = backslash;
  }

  if (rows.length === 0) notes.push("parsed 0 rows");
  if (!isContiguous(rows)) notes.push(`index not contiguous 1..${rows.length}`);
  if (rows.length !== VERSE_COUNT)
    notes.push(`row count ${rows.length} != canonical ${VERSE_COUNT}`);
  // Under backslash mode a decoded `\` can only come from a deliberate `\\` in
  // the source — verse text shouldn't contain any, so flag it as a guardrail.
  // (Standard mode may legitimately keep a lone `\`, so don't check there.)
  if (mode === "backslash") {
    const backslashRows = rows.filter((r) => r.text.includes("\\")).length;
    if (backslashRows > 0) notes.push(`${backslashRows} rows contain a backslash (check escaping)`);
  }

  let db: DatabaseSync | undefined;
  let integrity = "skipped";
  try {
    db = new DatabaseSync(outPath);
    db.exec("PRAGMA journal_mode = OFF");
    db.exec("PRAGMA synchronous = OFF");
    db.exec(`CREATE TABLE quran_text ("index" INTEGER PRIMARY KEY, sura INTEGER NOT NULL DEFAULT 0, aya INTEGER NOT NULL DEFAULT 0, text TEXT NOT NULL)`);
    db.exec("CREATE INDEX idx_quran_text_sura_aya ON quran_text (sura, aya)");

    const ins = db.prepare(`INSERT INTO quran_text ("index", sura, aya, text) VALUES (?, ?, ?, ?)`);
    db.exec("BEGIN");
    for (const r of rows) ins.run(r.index, r.sura, r.aya, r.text);
    db.exec("COMMIT");
    db.exec("VACUUM");

    integrity = String((db.prepare("PRAGMA integrity_check").get() as { integrity_check: string }).integrity_check);
  } finally {
    db?.close();
  }

  const countOk = rows.length === VERSE_COUNT;
  const ok = rows.length > 0 && countOk && integrity === "ok" && notes.length === 0;
  const sizeBytes = existsSync(outPath) ? statSync(outPath).size : 0;
  return { id, outPath, rows: rows.length, expected, ok, integrity, sizeBytes, sha256: sizeBytes > 0 ? fileSha256(outPath) : "", mode, notes };
}

async function main(args: string[]): Promise<number> {
  const index = await readJson<IndexFile>(INDEX);
  const trans = args.length
    ? index.translations.filter((t) => args.includes(t.id))
    : index.translations;

  if (trans.length === 0) {
    log(`no translations matched: ${args.join(", ")}`);
    return 1;
  }

  mkdirSync(OUTDIR, { recursive: true });
  log(`converting ${trans.length} translation(s) → ${path.relative(ROOT, OUTDIR)}/`);

  const results: BuildResult[] = [];
  const t0 = Date.now();
  for (const tr of trans) {
    const r = buildOne(tr.id, tr.ayaCount);
    results.push(r);
    const flag = r.ok ? "ok" : "WARN";
    const modeTag = r.mode === "standard" ? " [std]" : "";
    const extra = r.notes.length ? `  [${r.notes.join("; ")}]` : "";
    log(`  ${tr.id.padEnd(22)} ${String(r.rows).padStart(5)} rows  ${(r.sizeBytes / 1024).toFixed(0).padStart(5)} KB  ${r.integrity}  ${flag}${modeTag}${extra}`);
  }
  const dt = ((Date.now() - t0) / 1000).toFixed(1);

  const ok = results.filter((r) => r.ok).length;
  const bad = results.filter((r) => !r.ok);
  const std = results.filter((r) => r.mode === "standard").length;
  const totalBytes = results.reduce((s, r) => s + r.sizeBytes, 0);
  const rowTotal = results.reduce((s, r) => s + r.rows, 0);

  // Always write the digest manifest — even on partial failure — so catalog
  // and verify can report coherently. Only successful builds contribute digests;
  // a failed build means its id is absent and downstream gates fail loudly.
  const manifest: SqliteDigest[] = results
    .filter((r) => r.ok && r.sizeBytes > 0)
    .map((r) => ({ id: r.id, sizeBytes: r.sizeBytes, sha256: r.sha256 }))
    .sort((a, b) => a.id.localeCompare(b.id));
  await writeJson(MANIFEST, manifest);

  log("");
  log(`built ${ok}/${results.length} clean in ${dt}s  (escaping: ${results.length - std} backslash, ${std} standard)`);
  log(`total: ${rowTotal.toLocaleString()} rows, ${(totalBytes / (1024 * 1024)).toFixed(1)} MB`);
  log(`manifest → ${path.relative(ROOT, MANIFEST)} (${manifest.length} entries)`);
  if (bad.length) {
    log(`problems:`);
    for (const r of bad) log(`  ✗ ${r.id}: ${r.notes.join("; ") || `rows=${r.rows} expected=${r.expected} integrity=${r.integrity}`}`);
    return 1;
  }
  return 0;
}

main(process.argv.slice(2)).then((code) => process.exit(code));
