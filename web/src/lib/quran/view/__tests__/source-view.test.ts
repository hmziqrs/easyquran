import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { describe, expect, it } from "vite-plus/test";
import searchFixtures from "$lib/quran/search/__fixtures__/queries.json";
import prefixFixtures from "$lib/quran/view/__fixtures__/prefix-cuts.json";
import {
  OpenerKind,
  OpenerPackaging,
  QuranSourceId,
  type QuranSourceId as QuranSourceIdValue,
} from "$lib/data/quran-types";
import { buildCanonicalSearchCorpus, searchCanonicalCorpus } from "$lib/quran/search/corpus";
import { normalizeArabic, scalarLength } from "$lib/quran/search/normalize";
import { SearchHitKind, searchHitKey } from "$lib/quran/search/types";
import { createNodeQueryRunner } from "$lib/server/quran-node-query-runner";
import { sourceProfile } from "$lib/quran/view/source-profiles";
import { packagingCounts, scalarSlice, scalarToUtf16Index } from "$lib/quran/view/source-view";
import { loadQuranSource, readAllSourceRows } from "$lib/quran/view/source-runtime";
import { QURAN_DATA } from "$lib/server/quran-data";

function load(sourceId: QuranSourceIdValue) {
  const profile = sourceProfile(sourceId);
  const dbPath = path.resolve(process.cwd(), "..", profile.artifact.repositoryPath);
  const database = new DatabaseSync(dbPath);
  database.exec("PRAGMA query_only = ON");
  const runner = createNodeQueryRunner(database);
  const source = loadQuranSource(runner, profile, QURAN_DATA.coordinates);
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

  it.each([
    ["uthmani", uthmani],
    ["simple-clean", simple],
  ] as const)("matches every golden scalar-cut field for %s", (_name, source) => {
    const fixture = prefixFixtures.sources.find(
      (candidate) => candidate.sourceId === source.profile.sourceId,
    );
    expect(fixture?.sourceProfile).toBe(source.profile.id);
    expect(fixture?.surahs).toHaveLength(114);
    const firstBySurah = new Map(
      source.rows.filter((row) => row.ayah === 1).map((row) => [row.surah, row.text]),
    );
    for (const expected of fixture!.surahs) {
      const raw = firstBySurah.get(expected.surah)!;
      const normalization = source.view.normalization(expected.surah);
      expect({
        surah: normalization.surah,
        openerKind: normalization.openerKind,
        packaging: normalization.packaging,
        openerEndScalar: normalization.openerEndScalar,
        bodyStartScalar: normalization.bodyStartScalar,
        openerText: normalization.openerText,
        separator: scalarSlice(raw, normalization.openerEndScalar, normalization.bodyStartScalar),
        bodyPrefix20: Array.from(source.view.body(expected.surah, 1, raw))
          .slice(0, 20)
          .join(""),
      }).toEqual(expected);
    }
  });
});

describe("canonical web search corpus", () => {
  const corpus = buildCanonicalSearchCorpus({
    matchRows: simple.rows,
    displayRows: uthmani.rows,
    matchView: simple.view,
    displayView: uthmani.view,
  });

  it("normalizes pasted Uthmani text to match simple-clean", () => {
    const uthmaniBasmala = uthmani.view.opener(1).text!;
    expect(normalizeArabic(uthmaniBasmala)).toBe(normalizeArabic(simple.view.opener(1).text!));
  });

  it("matches every committed canonical search fixture", () => {
    for (const fixture of searchFixtures.fixtures) {
      const normalized = normalizeArabic(fixture.query);
      const result = searchCanonicalCorpus(corpus, fixture.query);
      expect({
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
      }).toEqual({
        normalized: fixture.normalized,
        normalizedScalarLength: fixture.normalizedScalarLength,
        total: fixture.total,
        first: fixture.first,
      });
    }
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
    expect(hits.filter((hit) => hit.kind === SearchHitKind.Ayah).map((hit) => searchHitKey(hit))).toEqual([
      "1:1",
      "27:30",
    ]);
  });

  it("does not match across the storage-only opener/body boundary", () => {
    const result = searchCanonicalCorpus(corpus, "الرحيم الم", { limit: 50 });
    expect(
      result.results.some((hit) => {
        const key = searchHitKey(hit);
        return key === "2:1" || key === "opener:2";
      }),
    ).toBe(false);
  });

  it("reports highlights against exact display strings and rebases ayah bodies", () => {
    const opener = searchCanonicalCorpus(corpus, uthmani.view.opener(1).text!, {
      limit: 10,
    }).results.find((hit) => searchHitKey(hit) === "opener:2");
    if (opener?.kind !== SearchHitKind.Opener) {
      throw new Error("missing opener:2 search fixture");
    }
    expect(opener.highlights).toEqual([{ start: 0, end: opener.text.length }]);

    const ayah = searchCanonicalCorpus(corpus, "الم", { limit: 50 }).results.find(
      (hit) => searchHitKey(hit) === "2:1",
    );
    expect(ayah?.kind).toBe(SearchHitKind.Ayah);
    if (ayah?.kind === SearchHitKind.Ayah) {
      expect(ayah.highlights).toEqual([
        {
          start: scalarToUtf16Index(ayah.ayah.text, uthmani.view.normalization(2).bodyStartScalar),
          end: ayah.ayah.text.length,
        },
      ]);
    }
  });

  it("maps simple-clean matches onto different Uthmani spellings", () => {
    const result = searchCanonicalCorpus(corpus, "سبحان الله", { limit: 50 });
    expect(result.total).toBe(9);
    expect(result.results).toHaveLength(result.total);
    for (const hit of result.results) {
      expect(hit.highlights.length).toBeGreaterThan(0);
      const text = hit.kind === SearchHitKind.Opener ? hit.text : hit.ayah.text;
      for (const highlight of hit.highlights) {
        expect(highlight.start).toBeGreaterThanOrEqual(0);
        expect(highlight.end).toBeGreaterThan(highlight.start);
        expect(highlight.end).toBeLessThanOrEqual(text.length);
      }
    }
  });
});
