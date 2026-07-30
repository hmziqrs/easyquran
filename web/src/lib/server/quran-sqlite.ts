/* ════════════════════════════════════════════════════════════════════════
   quran-sqlite.ts — SERVER-ONLY Uthmani verse reader for SSG.

   Opens the checked-in db/quran/tanzil/arabic/quran-uthmani.sqlite through
   Node's built-in node:sqlite (v24) and serves verbatim Uthmani verse text to
   the prerender load (+page.server.ts). It never runs in the browser — the
   browser reads the same file via sqlite-wasm in a Worker (Phase 2). Lives under
   $lib/server so SvelteKit guarantees it is stripped from the client bundle.
   ════════════════════════════════════════════════════════════════════════ */

import { DatabaseSync, type StatementSync } from "node:sqlite";
import { existsSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { QURAN } from "$lib/config/site";
import type { Ayah } from "$lib/data/quran-types";

/**
 * Resolve the checked-in Uthmani DB from `process.cwd()`. SSR bundling relocates
 * modules into .svelte-kit/output, so `import.meta.url` traversal is unreliable
 * here; cwd is the package dir during `vp dev` / `vp build` (or the repo root),
 * and we accept either.
 */
function findUthmaniPath(): string {
  const candidates = [
    path.resolve(process.cwd(), "db/quran/tanzil/arabic/quran-uthmani.sqlite"), // cwd = repo root
    path.resolve(process.cwd(), "../db/quran/tanzil/arabic/quran-uthmani.sqlite"), // cwd = web/
  ];
  return candidates.find((p) => existsSync(p)) ?? candidates[1]!;
}
const UTHMANI_PATH = findUthmaniPath();

const EXPECTED_ROWS = 6236;
/** quran-api.md §3.3 golden digest for the Uthmani corpus bytes. */
const EXPECTED_SHA256 = QURAN.scripts.find((s) => s.id === "uthmani")!.sha256;

let db: DatabaseSync | null = null;
const suraStmtCache = new Map<number, StatementSync>();

function getDb(): DatabaseSync {
  if (db) return db;
  if (!existsSync(UTHMANI_PATH)) {
    throw new Error(`[quran-sqlite] missing Uthmani DB at ${UTHMANI_PATH}`);
  }
  const handle = new DatabaseSync(UTHMANI_PATH);
  // Read-only guard: any write throws, even if a future query mutates.
  handle.exec("PRAGMA query_only = ON");
  db = handle;
  return handle;
}

/** Verbatim Uthmani verse text for one surah, in ayah order (1..ayahCount). */
export function readSurahVerses(num: number): string[] {
  let stmt = suraStmtCache.get(num);
  if (!stmt) {
    stmt = getDb().prepare(
      `SELECT text FROM quran_text WHERE sura = ? ORDER BY aya`,
    );
    suraStmtCache.set(num, stmt);
  }
  const rows = stmt.all(num) as { text: string }[];
  return rows.map((r) => r.text);
}

/** Verbatim Uthmani ayahs in an inclusive global-index range (juz / page). */
let rangeStmt: StatementSync | null = null;
export function readRangeAyahs(from: number, to: number): Ayah[] {
  if (!rangeStmt) {
    rangeStmt = getDb().prepare(
      `SELECT "index", sura, aya, text FROM quran_text WHERE "index" BETWEEN ? AND ? ORDER BY "index"`,
    );
  }
  const rows = rangeStmt.all(from, to) as {
    index: number;
    sura: number;
    aya: number;
    text: string;
  }[];
  return rows.map((r) => ({
    key: `${r.sura}:${r.aya}`,
    surah: r.sura,
    ayah: r.aya,
    globalIndex: r.index,
    text: r.text,
  }));
}

/** Validate the source file: identity byte digest + row count. Called in dev /
 *  at build start to catch drift before prerendering 114 pages. */
export function validateUthmani(): { rows: number; sha256: string; ok: boolean } {
  const buf = readFileSync(UTHMANI_PATH);
  const sha256 = createHash("sha256").update(buf).digest("hex");
  const { c } = getDb().prepare(`SELECT count(*) AS c FROM quran_text`).get() as { c: number };
  const ok = c === EXPECTED_ROWS && sha256 === EXPECTED_SHA256;
  return { rows: c, sha256, ok };
}
