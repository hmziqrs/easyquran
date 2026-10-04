/**
 * Build db/quran/arabic/quran-uthmani-annotated.sqlite — the fully annotated
 * Uthmani mushaf text (small-meem pausals U+06E2/U+06ED, stop marks over the
 * waqf medallions, sajdah signs) that quran.com's reader pages render.
 *
 * Source: quran.com public API v4 word-level `text_uthmani` (Quran Foundation /
 * QUL mushaf word lineage). The API's VERSE-level `text_uthmani` is a stripped
 * lineage identical to our Tanzil DB — the annotations live only on the words,
 * so the verse text is rebuilt by joining word tokens (char_type_name "word",
 * single space; "end" tokens are ayah-number glyphs and are dropped).
 *
 * One-shot, deterministic, network fetch at build time. Identity = the plain id
 * string below + byte size — repo hard rule: never a SHA-256 over Quran data.
 * Every other DB under db/ is immutable: this tool writes ONLY the new artifact
 * (and reads the Tanzil DB read-only for the F4 adjudication report).
 *
 * Usage: pnpm exec tsx scripts/quran/build-uthmani-annotated.ts [--out PATH]
 */

// Repo-root `vp check` type-checks with tsgolint, which does not auto-include
// node_modules/@types the way `tsc -p scripts/quran/tsconfig.json` does; this
// reference pulls the Node typings in under both checkers. No runtime effect.
/// <reference types="node" />

import { mkdirSync, rmSync, statSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const DEFAULT_OUT = path.join(REPO_ROOT, "db", "quran", "arabic", "quran-uthmani-annotated.sqlite");
const TANZIL_DB = path.join(REPO_ROOT, "db", "quran", "arabic", "quran-uthmani.sqlite");
const API_BASE = "https://api.quran.com/api/v4/verses/by_chapter";
const HEADERS = Object.freeze({
  "User-Agent": "easyquran-artifact-builder",
  Accept: "application/json",
});

/** Artifact identity — plain id string, never a hash. */
const ARTIFACT_ID = "uthmani-annotated-qcom-words-v1";
const TOTAL_ROWS = 6236;
const SQLITE_PAGE_SIZE = 4096;

interface ApiWord {
  readonly char_type_name: string;
  readonly text_uthmani: string;
}

interface ApiVerse {
  readonly verse_key: string;
  readonly words: readonly ApiWord[];
}

function log(message: string): void {
  process.stdout.write(`${message}\n`);
}

function parseArgs(): { outPath: string } {
  const args = process.argv.slice(2);
  const supported = new Set(["--", "--out"]);
  const isOutValue = (index: number): boolean => args[index - 1] === "--out";
  const unknown = args.filter(
    (arg, index) => !isOutValue(index) && !supported.has(arg) && !arg.startsWith("--out="),
  );
  if (unknown.length > 0) throw new Error(`unknown option(s): ${unknown.join(", ")}`);
  const outFlag = args.find((arg) => arg.startsWith("--out="));
  const outIndex = args.indexOf("--out");
  if (outFlag) return { outPath: outFlag.slice("--out=".length) };
  if (outIndex !== -1) {
    const value = args[outIndex + 1];
    if (!value) throw new Error("--out requires a path");
    return { outPath: value };
  }
  return { outPath: DEFAULT_OUT };
}

async function fetchJson(url: string): Promise<unknown> {
  for (let attempt = 1; attempt <= 4; attempt++) {
    try {
      const response = await fetch(url, { headers: HEADERS });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return await response.json();
    } catch (error) {
      if (attempt === 4) throw new Error(`fetch failed (${url}): ${String(error)}`);
      await new Promise((resolve) => setTimeout(resolve, 1000 * 2 ** (attempt - 1)));
    }
  }
  throw new Error("unreachable");
}

function parseVerses(payload: unknown, surah: number): readonly ApiVerse[] {
  if (typeof payload !== "object" || payload === null)
    throw new Error(`surah ${surah}: bad payload`);
  const verses = (payload as { verses?: unknown }).verses;
  if (!Array.isArray(verses)) throw new Error(`surah ${surah}: missing verses array`);
  return verses as readonly ApiVerse[];
}

function joinVerseText(verse: ApiVerse, verseKey: string): string {
  const parts: string[] = [];
  for (const word of verse.words) {
    if (word.char_type_name === "word") {
      const text = word.text_uthmani;
      if (typeof text !== "string" || text.length === 0) {
        throw new Error(`${verseKey}: empty word token`);
      }
      parts.push(text);
    } else if (word.char_type_name !== "end") {
      throw new Error(`${verseKey}: unexpected char_type_name ${word.char_type_name}`);
    }
  }
  if (parts.length === 0) throw new Error(`${verseKey}: no word tokens`);
  return parts.join(" ").replace(/^\s+|\s+$/gu, "");
}

interface Row {
  readonly sura: number;
  readonly aya: number;
  readonly text: string;
}

async function fetchAllVerses(): Promise<readonly Row[]> {
  const rows: Row[] = [];
  let expectedSurah = 1;
  let expectedAya = 1;
  for (let surah = 1; surah <= 114; surah++) {
    const payload = await fetchJson(
      `${API_BASE}/${surah}?words=true&word_fields=text_uthmani&per_page=300`,
    );
    const verses = parseVerses(payload, surah);
    for (const verse of verses) {
      const key = verse.verse_key;
      if (key !== `${surah}:${expectedAya}`) {
        throw new Error(`tiling break: expected ${surah}:${expectedAya}, got ${key}`);
      }
      rows.push({ sura: expectedSurah, aya: expectedAya, text: joinVerseText(verse, key) });
      expectedAya += 1;
    }
    expectedSurah += 1;
    expectedAya = 1;
    log(`fetched surah ${surah} (${verses.length} verses)`);
  }
  if (rows.length !== TOTAL_ROWS) throw new Error(`row count ${rows.length} != ${TOTAL_ROWS}`);
  return rows;
}

function writeDatabase(outPath: string, rows: readonly Row[]): void {
  mkdirSync(path.dirname(outPath), { recursive: true });
  rmSync(outPath, { force: true });
  rmSync(`${outPath}-wal`, { force: true });
  rmSync(`${outPath}-shm`, { force: true });
  const database = new DatabaseSync(outPath);
  try {
    database.exec(`PRAGMA page_size = ${SQLITE_PAGE_SIZE};`);
    database.exec(`PRAGMA encoding = 'UTF-8';`);
    database.exec(
      'CREATE TABLE quran_text ("index" INTEGER PRIMARY KEY, sura INTEGER NOT NULL DEFAULT 0, aya INTEGER NOT NULL DEFAULT 0, text TEXT NOT NULL);',
    );
    database.exec("CREATE INDEX idx_quran_text_sura_aya ON quran_text (sura, aya);");
    const insert = database.prepare(
      'INSERT INTO quran_text ("index", sura, aya, text) VALUES (?, ?, ?, ?)',
    );
    database.exec("BEGIN");
    rows.forEach((row, index) => {
      insert.run(index + 1, row.sura, row.aya, row.text);
    });
    database.exec("COMMIT");
  } finally {
    database.close();
  }
}

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(`verify failed: ${message}`);
}

function verifyDatabase(outPath: string): void {
  const database = new DatabaseSync(outPath, { readOnly: true });
  try {
    const count = database.prepare("SELECT COUNT(*) AS n FROM quran_text").get() as { n: number };
    assert(count.n === TOTAL_ROWS, `row count ${count.n}`);
    const textAt = (sura: number, aya: number): string => {
      const row = database
        .prepare("SELECT text FROM quran_text WHERE sura = ? AND aya = ?")
        .get(sura, aya) as {
        text: string;
      };
      assert(typeof row?.text === "string", `missing ${sura}:${aya}`);
      return row.text;
    };
    const first = textAt(1, 1);
    assert(first.startsWith("بِسْمِ"), `1:1 opener text: ${first}`);
    const opener = textAt(2, 1);
    assert(opener === "الٓمٓ", `2:1 must not embed bismillah: ${opener}`);
    assert(textAt(106, 4).includes("\u06e2"), "106:4 waqf-lazim small meem U+06E2 missing");
    assert(textAt(94, 6).includes("\u06ed"), "94:6 stop mark U+06ED missing");
    assert(textAt(32, 15).includes("\u06e9"), "32:15 sajdah sign missing");
    const maddahVerse = textAt(80, 3);
    assert(maddahVerse.includes("\u0653"), "80:3 maddah U+0653 missing");
  } finally {
    database.close();
  }
}

/** F4 adjudication: earlier capture claimed 80:3 lost the maddah; codepoint compare said 0 diffs. */
function adjudicate803(outPath: string): void {
  const annotated = new DatabaseSync(outPath, { readOnly: true });
  const tanzil = new DatabaseSync(TANZIL_DB, { readOnly: true });
  try {
    const read = (database: DatabaseSync): string => {
      const row = database
        .prepare("SELECT text FROM quran_text WHERE sura = 80 AND aya = 3")
        .get() as {
        text: string;
      };
      return row.text;
    };
    const ours = read(tanzil);
    const theirs = read(annotated);
    const codepoints = (text: string): string =>
      Array.from(text)
        .map((character) => `U+${character.codePointAt(0)?.toString(16).padStart(4, "0")}`)
        .join(" ");
    log(`adjudication 80:3 tanzil    : ${codepoints(ours.slice(-6))}`);
    log(`adjudication 80:3 annotated : ${codepoints(theirs.slice(-6))}`);
    log(
      ours.includes("\u0653")
        ? "verdict: Tanzil 80:3 DOES carry U+0653 maddah on يَزَّكَّىٰٓ — the dropped-maddah capture was wrong; the 0-diff codepoint compare is correct."
        : "verdict: Tanzil 80:3 lacks U+0653 — dropped-maddah capture stands.",
    );
  } finally {
    annotated.close();
    tanzil.close();
  }
}

async function main(): Promise<void> {
  const { outPath } = parseArgs();
  log(`building ${path.relative(REPO_ROOT, outPath)} (id ${ARTIFACT_ID})`);
  const rows = await fetchAllVerses();
  writeDatabase(outPath, rows);
  verifyDatabase(outPath);
  adjudicate803(outPath);
  const sizeBytes = statSync(outPath).size;
  log(
    `done: ${TOTAL_ROWS} rows, sizeBytes ${sizeBytes} — bake into source-profiles.ts artifact.sizeBytes`,
  );
}

await main();
