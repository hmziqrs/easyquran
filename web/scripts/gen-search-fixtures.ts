/**
 * gen-search-fixtures.ts — emit the shared search fixture suite.
 *
 * Reads the simple-clean corpus, applies the REAL normalize.ts (so the fixtures
 * can never drift from the implementation), runs a fixed query set, and writes
 * the first-N ordered verse keys + totals to __fixtures__/queries.json. The
 * same file is consumed by the Rust API's CI (docs/quran-api.md §7.3) so online
 * and offline search must agree.
 *
 *   node --experimental-strip-types web/scripts/gen-search-fixtures.ts
 */
import { DatabaseSync } from "node:sqlite";
import { writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_LIMIT,
  normalizeArabic,
  scalarLength,
} from "../src/lib/quran/search/normalize.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(__dirname, "..");
const SC = path.join(WEB, "../db/quran/tanzil/arabic/quran-simple-clean.sqlite");
const OUT = path.join(WEB, "src/lib/quran/search/__fixtures__/queries.json");

const db = new DatabaseSync(SC);
db.exec("PRAGMA query_only = ON");
const rows = db
  .prepare('SELECT "index", sura, aya, text FROM quran_text ORDER BY "index"')
  .all() as { index: number; sura: number; aya: number; text: string }[];

const corpus = rows.map((r) => ({
  index: r.index,
  sura: r.sura,
  aya: r.aya,
  norm: normalizeArabic(r.text),
}));

const QUERIES = [
  "بسم الله",
  "الحمد لله",
  "إن الله",
  "جنة",
  "الناس",
];

interface Fixture {
  query: string;
  normalized: string;
  normalizedScalarLength: number;
  total: number;
  first: { key: string; surah: number; ayah: number; globalIndex: number }[];
}

const fixtures: Fixture[] = QUERIES.map((query) => {
  const norm = normalizeArabic(query);
  const first: Fixture["first"] = [];
  let total = 0;
  for (const row of corpus) {
    if (row.norm.includes(norm)) {
      total++;
      if (first.length < DEFAULT_LIMIT) {
        first.push({ key: `${row.sura}:${row.aya}`, surah: row.sura, ayah: row.aya, globalIndex: row.index });
      }
    }
  }
  return { query, normalized: norm, normalizedScalarLength: scalarLength(norm), total, first };
});

mkdirSync(path.dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify({ searchVersion: "arabic-search-v1", fixtures }, null, 2) + "\n");
console.log(
  `wrote ${OUT}\n  ` +
    fixtures.map((f) => `"${f.query}" → ${f.total} match(es)`).join("\n  "),
);
