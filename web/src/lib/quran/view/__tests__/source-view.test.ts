import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { describe, expect, it } from "vite-plus/test";
import {
  OpenerKind,
  OpenerPackaging,
  QuranSourceId,
  type QuranSourceId as QuranSourceIdValue,
} from "$lib/data/quran-types";
import { buildCanonicalSearchCorpus, searchCanonicalCorpus } from "$lib/quran/search/corpus";
import { SEARCH_VERSION, SearchHitKind, normalizeArabic } from "$lib/quran/search/normalize";
import { createNodeQueryRunner } from "$lib/server/quran-node-query-runner";
import { sourceProfile } from "$lib/quran/view/source-profiles";
import { packagingCounts, scalarSlice, scalarToUtf16Index } from "$lib/quran/view/source-view";
import { loadQuranSource, readAllSourceRows } from "$lib/quran/view/source-runtime";

function load(sourceId: QuranSourceIdValue) {
  const profile = sourceProfile(sourceId);
  const dbPath = path.resolve(process.cwd(), "..", profile.artifact.repositoryPath);
  const database = new DatabaseSync(dbPath);
  database.exec("PRAGMA query_only = ON");
  const runner = createNodeQueryRunner(database);
  const source = loadQuranSource(runner, profile);
  const rows = readAllSourceRows(runner, source);
  database.close();
  return { profile, rows, view: source.view };
}

const uthmani = load(QuranSourceId.TanzilUthmani);
const simple = load(QuranSourceId.TanzilSimpleClean);

describe("registered Quran source views", () => {
  it.each([
    ["uthmani", uthmani],
    ["simple-clean", simple],
  ] as const)("losslessly partitions every embedded %s first ayah", (_name, source) => {
    const firstBySurah = new Map(
      source.rows.filter((row) => row.ayah === 1).map((row) => [row.surah, row.text]),
    );
    expect(packagingCounts(source.view.normalizations())).toEqual(
      source.profile.expectedPackagingCounts,
    );
    for (let surah = 1; surah <= 114; surah++) {
      const raw = firstBySurah.get(surah)!;
      const descriptor = source.view.normalization(surah);
      const body = source.view.body(surah, 1, raw);
      expect(body.length).toBeGreaterThan(0);
      expect(body.trim()).toBe(body);
      expect(source.view.raw(raw)).toBe(raw);
      if (descriptor.packaging === OpenerPackaging.EmbeddedPrefix) {
        const opener = scalarSlice(raw, 0, descriptor.openerEndScalar);
        const separator = scalarSlice(raw, descriptor.openerEndScalar, descriptor.bodyStartScalar);
        expect(opener + separator + body).toBe(raw);
        expect(separator).toBe(" ");
      } else {
        expect(body).toBe(raw);
      }
    }
  });

  it("preserves the Uthmani 95/97 shadda and source-specific cuts", () => {
    expect(new Set(uthmani.view.normalizations().map((value) => value.openerEndScalar))).toEqual(
      new Set([0, 39, 40]),
    );
    expect(uthmani.view.normalization(95).bodyStartScalar).toBe(41);
    expect(uthmani.view.normalization(97).bodyStartScalar).toBe(41);
    expect(uthmani.view.opener(95).text?.startsWith("بِّسْمِ")).toBe(true);
    expect(simple.view.normalization(95).bodyStartScalar).toBe(23);
  });

  it("preserves Al-Fatihah as a numbered verse and At-Tawbah as no opener", () => {
    expect(uthmani.view.normalization(1).bodyStartScalar).toBe(0);
    expect(uthmani.view.opener(1).kind).toBe(OpenerKind.Verse);
    expect(uthmani.view.opener(9)).toEqual({ kind: OpenerKind.None });
  });
});

describe("canonical web search corpus", () => {
  const corpus = buildCanonicalSearchCorpus({
    matchRows: simple.rows,
    displayRows: uthmani.rows,
    matchView: simple.view,
    displayView: uthmani.view,
  });

  it("uses v2 normalization for pasted Uthmani text", () => {
    const uthmaniBasmala = uthmani.view.opener(1).text!;
    expect(SEARCH_VERSION).toBe("arabic-search-v2");
    expect(normalizeArabic(uthmaniBasmala)).toBe(normalizeArabic(simple.view.opener(1).text!));
  });

  it("attributes the full basmala to 112 openers and two numbered ayahs", () => {
    const result = searchCanonicalCorpus(corpus, uthmani.view.opener(1).text!, { limit: 50 });
    expect(result.total).toBe(114);
    const all = searchCanonicalCorpus(corpus, uthmani.view.opener(1).text!, {
      limit: 50,
      offset: 50,
    });
    const last = searchCanonicalCorpus(corpus, uthmani.view.opener(1).text!, {
      limit: 50,
      offset: 100,
    });
    const hits = [...result.results, ...all.results, ...last.results];
    expect(hits.filter((hit) => hit.kind === SearchHitKind.Opener)).toHaveLength(112);
    expect(hits.filter((hit) => hit.kind === SearchHitKind.Ayah).map((hit) => hit.key)).toEqual([
      "1:1",
      "27:30",
    ]);
  });

  it("does not match across the storage-only opener/body boundary", () => {
    const result = searchCanonicalCorpus(corpus, "الرحيم الم", { limit: 50 });
    expect(result.results.some((hit) => hit.key === "2:1" || hit.key === "opener:2")).toBe(false);
  });

  it("reports highlights against exact display strings and rebases ayah bodies", () => {
    const opener = searchCanonicalCorpus(corpus, uthmani.view.opener(1).text!, {
      limit: 10,
    }).results.find((hit) => hit.key === "opener:2");
    if (!opener) throw new Error("missing opener:2 search fixture");
    expect(opener.highlights).toEqual([{ start: 0, end: opener.text.length }]);

    const ayah = searchCanonicalCorpus(corpus, "الم", { limit: 50 }).results.find(
      (hit) => hit.key === "2:1",
    );
    expect(ayah?.kind).toBe(SearchHitKind.Ayah);
    if (ayah?.kind === SearchHitKind.Ayah) {
      expect(ayah.highlights).toEqual([
        {
          start: scalarToUtf16Index(ayah.text, uthmani.view.normalization(2).bodyStartScalar),
          end: ayah.text.length,
        },
      ]);
    }
  });
});
