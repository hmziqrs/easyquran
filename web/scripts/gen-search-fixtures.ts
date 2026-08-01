/** Generate web/Rust shared canonical search fixtures from registered sources. */
import { DatabaseSync } from "node:sqlite";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { QuranSourceId } from "../src/lib/data/quran-types.ts";
import { DEFAULT_QURAN_SOURCE_PLAN } from "../src/lib/quran/source-plan.ts";
import { createNodeQueryRunner } from "../src/lib/server/quran-node-query-runner.ts";
import {
  buildCanonicalSearchCorpus,
  searchCanonicalCorpus,
} from "../src/lib/quran/search/corpus.ts";
import {
  DEFAULT_LIMIT,
  SEARCH_VERSION,
  SearchHitKind,
  normalizeArabic,
  scalarLength,
} from "../src/lib/quran/search/normalize.ts";
import { sourceProfile } from "../src/lib/quran/view/source-profiles.ts";
import { loadQuranSource, readAllSourceRows } from "../src/lib/quran/view/source-runtime.ts";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(scriptDir, "..");
const OUT = path.join(WEB, "src/lib/quran/search/__fixtures__/queries.json");

function load(sourceId: QuranSourceId) {
  const profile = sourceProfile(sourceId);
  const database = new DatabaseSync(path.join(WEB, "..", profile.artifact.repositoryPath));
  database.exec("PRAGMA query_only = ON");
  const runner = createNodeQueryRunner(database);
  const source = loadQuranSource(runner, profile);
  const rows = readAllSourceRows(runner, source);
  database.close();
  return { rows, view: source.view };
}

const match = load(DEFAULT_QURAN_SOURCE_PLAN.search.match);
const display = load(DEFAULT_QURAN_SOURCE_PLAN.search.display);
const corpus = buildCanonicalSearchCorpus({
  matchRows: match.rows,
  displayRows: display.rows,
  matchView: match.view,
  displayView: display.view,
});

const fullDisplayBasmala = display.view.opener(1).text!;
const QUERIES = ["بسم الله", "الحمد لله", "إن الله", "جنة", "الناس", fullDisplayBasmala];

const fixtures = QUERIES.map((query) => {
  const normalized = normalizeArabic(query);
  const result = searchCanonicalCorpus(corpus, query, { limit: DEFAULT_LIMIT });
  return {
    query,
    normalized,
    normalizedScalarLength: scalarLength(normalized),
    total: result.total,
    first: result.results.map((hit) =>
      hit.kind === SearchHitKind.Opener
        ? {
            kind: hit.kind,
            key: hit.key,
            surah: hit.surah,
            anchorAyah: hit.anchorAyah,
          }
        : {
            kind: hit.kind,
            key: hit.key,
            surah: hit.surah,
            ayah: hit.ayah,
            globalIndex: hit.globalIndex,
          },
    ),
  };
});

mkdirSync(path.dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify({ searchVersion: SEARCH_VERSION, fixtures }, null, 2) + "\n");
console.log(
  `wrote ${OUT}\n  ` +
    fixtures.map((fixture) => `"${fixture.query}" → ${fixture.total}`).join("\n  "),
);
