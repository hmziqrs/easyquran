/** Generate canonical search and source-view fixtures from registered sources. */
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
  normalizeArabic,
  scalarLength,
} from "../src/lib/quran/search/normalize.ts";
import { SearchHitKind } from "../src/lib/quran/search/types.ts";
import { registeredSourceProfiles, sourceProfile } from "../src/lib/quran/view/source-profiles.ts";
import { scalarSlice } from "../src/lib/quran/view/source-view.ts";
import { loadQuranSource, readAllSourceRows } from "../src/lib/quran/view/source-runtime.ts";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(scriptDir, "..");
const SEARCH_OUT = path.join(WEB, "src/lib/quran/search/__fixtures__/queries.json");
const VIEW_OUT = path.join(WEB, "src/lib/quran/view/__fixtures__/prefix-cuts.json");

function load(sourceId: QuranSourceId) {
  const profile = sourceProfile(sourceId);
  const database = new DatabaseSync(path.join(WEB, "..", profile.artifact.repositoryPath));
  database.exec("PRAGMA query_only = ON");
  const runner = createNodeQueryRunner(database);
  const source = loadQuranSource(runner, profile);
  const rows = readAllSourceRows(runner, source);
  database.close();
  return { profile, rows, view: source.view };
}

const loadedSources = new Map(
  registeredSourceProfiles().map((profile) => [profile.sourceId, load(profile.sourceId)]),
);
const match = loadedSources.get(DEFAULT_QURAN_SOURCE_PLAN.search.match)!;
const display = loadedSources.get(DEFAULT_QURAN_SOURCE_PLAN.search.display)!;
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
            key: hit.ayah.key,
            surah: hit.ayah.surah,
            ayah: hit.ayah.ayah,
            globalIndex: hit.ayah.globalIndex,
          },
    ),
  };
});

const sourceViews = [...loadedSources.values()].map((source) => {
  const firstBySurah = new Map(
    source.rows.filter((row) => row.ayah === 1).map((row) => [row.surah, row.text]),
  );
  return {
    sourceId: source.profile.sourceId,
    sourceProfile: source.profile.id,
    surahs: source.view.normalizations().map((normalization) => {
      const raw = firstBySurah.get(normalization.surah)!;
      const body = source.view.body(normalization.surah, 1, raw);
      return {
        surah: normalization.surah,
        openerKind: normalization.openerKind,
        packaging: normalization.packaging,
        openerEndScalar: normalization.openerEndScalar,
        bodyStartScalar: normalization.bodyStartScalar,
        openerText: normalization.openerText,
        separator: scalarSlice(raw, normalization.openerEndScalar, normalization.bodyStartScalar),
        bodyPrefix20: Array.from(body).slice(0, 20).join(""),
      };
    }),
  };
});

mkdirSync(path.dirname(SEARCH_OUT), { recursive: true });
mkdirSync(path.dirname(VIEW_OUT), { recursive: true });
writeFileSync(
  SEARCH_OUT,
  JSON.stringify({ searchVersion: SEARCH_VERSION, fixtures }, null, 2) + "\n",
);
writeFileSync(VIEW_OUT, JSON.stringify({ sources: sourceViews }, null, 2) + "\n");
console.log(
  `wrote ${SEARCH_OUT}\nwrote ${VIEW_OUT}\n  ` +
    fixtures.map((fixture) => `"${fixture.query}" → ${fixture.total}`).join("\n  "),
);
