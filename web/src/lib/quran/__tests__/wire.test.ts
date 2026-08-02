import { describe, expect, it } from "vite-plus/test";
import { OpenerKind, OpenerPackaging, QuranScript, QuranSourceId } from "$lib/data/quran-types";
import { SearchHitKind, type SearchHit } from "$lib/quran/search/types";
import {
  decodeQuranSurahText,
  decodeScript,
  decodeScriptsPayload,
  decodeSearchHit,
  decodeSearchResponse,
  decodeVersionPayload,
  unwrapEnvelope,
} from "$lib/quran/wire";
import { QURAN_CATALOG } from "$lib/server/quran-metadata";

const validateCoordinate = (globalIndex: number, surah: number, ayah: number): boolean =>
  QURAN_CATALOG.globalIndexOf(surah, ayah) === globalIndex;

const AYAH_HIT: SearchHit = {
  kind: SearchHitKind.Ayah,
  ayah: {
    key: "2:255",
    surah: 2,
    ayah: 255,
    globalIndex: 262,
    text: "اللَّهُ لاَ إِلَـٰهَ إِلاَّ هُوَ",
  },
  highlights: [{ start: 0, end: 7 }],
};

const OPENER_HIT: SearchHit = {
  kind: SearchHitKind.Opener,
  key: "opener:2",
  surah: 2,
  anchorAyah: 1,
  text: "بِسْمِ ٱللَّهِ",
  highlights: [{ start: 0, end: 14 }],
};

describe("unwrapEnvelope", () => {
  it("unwraps data but preserves nullish and non-object bodies", () => {
    expect(unwrapEnvelope({ data: { results: [] } })).toEqual({ results: [] });
    const nullish = { data: null, extra: 1 };
    expect(unwrapEnvelope(nullish)).toBe(nullish);
    expect(unwrapEnvelope("body")).toBe("body");
  });
});

describe("canonical search wire", () => {
  it.each([AYAH_HIT, OPENER_HIT])("rebuilds a tagged hit", (value) => {
    const decoded = decodeSearchHit(value, validateCoordinate);
    expect(decoded).toEqual(value);
    expect(decoded).not.toBe(value);
  });

  it("rejects legacy ayah-only and malformed tagged shapes", () => {
    expect(decodeSearchHit({ surah: 2, ayah: 1, highlights: [] }, validateCoordinate)).toBeNull();
    expect(
      decodeSearchHit({ ...AYAH_HIT, ayah: { ...AYAH_HIT.ayah, surah: "2" } }, validateCoordinate),
    ).toBeNull();
    expect(
      decodeSearchHit(
        { ...AYAH_HIT, ayah: { ...AYAH_HIT.ayah, globalIndex: 263 } },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(
      decodeSearchHit({ ...AYAH_HIT, highlights: [{ start: 4, end: 2 }] }, validateCoordinate),
    ).toBeNull();
    expect(decodeSearchHit({ ...OPENER_HIT, anchorAyah: 2 }, validateCoordinate)).toBeNull();
  });

  it("decodes a response and fails closed on malformed individual hits", () => {
    expect(
      decodeSearchResponse(
        {
          query: "الله",
          total: 2,
          limit: 20,
          offset: 0,
          results: [AYAH_HIT, OPENER_HIT],
        },
        validateCoordinate,
      ),
    ).toEqual({
      query: "الله",
      total: 2,
      limit: 20,
      offset: 0,
      results: [AYAH_HIT, OPENER_HIT],
    });
    expect(
      decodeSearchResponse(
        {
          query: "الله",
          total: 2,
          limit: 20,
          offset: 0,
          results: [AYAH_HIT, { ...OPENER_HIT, anchorAyah: 2 }, OPENER_HIT],
        },
        validateCoordinate,
      ),
    ).toBeNull();
    expect(decodeSearchResponse({ results: "invalid" })).toBeNull();
  });
});

describe("normalized surah Worker wire", () => {
  const payload = {
    sourceId: QuranSourceId.TanzilUthmani,
    script: QuranScript.Uthmani,
    verses: ["raw first", "raw second"],
    normalization: {
      surah: 2,
      sourceId: QuranSourceId.TanzilUthmani,
      script: QuranScript.Uthmani,
      sourceProfile: "tanzil-uthmani-581cc540",
      packaging: OpenerPackaging.EmbeddedPrefix,
      openerKind: OpenerKind.Header,
      openerText: "opener",
      openerEndScalar: 6,
      bodyStartScalar: 7,
    },
  };

  it("rebuilds raw verses and their descriptor", () => {
    expect(decodeQuranSurahText(payload)).toEqual(payload);
  });

  it("rejects a mismatched source identity, script, or scalar cut", () => {
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, sourceProfile: "unregistered" },
      }),
    ).toBeNull();
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, script: QuranScript.SimpleClean },
      }),
    ).toBeNull();
    expect(
      decodeQuranSurahText({
        ...payload,
        normalization: { ...payload.normalization, openerEndScalar: 8 },
      }),
    ).toBeNull();
  });
});

describe("manifest wire", () => {
  const scripts = [
    { id: QuranSourceId.TanzilUthmani, sizeBytes: 1, sha256: "a", downloadUrl: "https://x/u" },
    { id: QuranSourceId.TanzilSimpleClean, sizeBytes: 2, sha256: "b", downloadUrl: "https://x/s" },
  ];

  it("decodes registered script ids and script envelopes", () => {
    expect(decodeScript(scripts[0])).toEqual(scripts[0]);
    expect(decodeScript({ ...scripts[0], id: "unknown" })).toBeNull();
    expect(decodeScriptsPayload({ data: { scripts } })).toEqual(scripts);
  });

  it("decodes version envelopes with nullable missing fields", () => {
    expect(decodeVersionPayload({ data: { contentVersion: "c", searchVersion: "s" } })).toEqual({
      contentVersion: "c",
      searchVersion: "s",
    });
    expect(decodeVersionPayload({ contentVersion: "c" })).toEqual({
      contentVersion: "c",
      searchVersion: null,
    });
  });
});
