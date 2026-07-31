import { describe, expect, it } from "vite-plus/test";
import type { SearchHit } from "$lib/quran/search/normalize";
import {
  decodeScript,
  decodeScriptsPayload,
  decodeSearchHit,
  decodeSearchResponse,
  decodeVersionPayload,
  unwrapEnvelope,
} from "$lib/quran/wire";

/* The shared wire decoders guard every trust boundary (HTTP API + worker
 * postMessage). These tests pin each shape: valid payloads rebuild exactly,
 * malformed shapes yield null, wrong types are rejected (never coerced), and
 * the `{ data }` envelope is stripped uniformly. */

const VALID_HIT: SearchHit = {
  key: "2:255",
  surah: 2,
  ayah: 255,
  globalIndex: 2623,
  text: "اللَّهُ لاَ إِلَـٰهَ إِلاَّ هُوَ الْحَيُّ الْقَيُّومُ",
};

describe("unwrapEnvelope", () => {
  it("returns the inner data when enveloped", () => {
    expect(unwrapEnvelope({ data: { results: [] } })).toEqual({ results: [] });
  });

  it("returns the body when data is absent", () => {
    const body = { results: [1], total: 1 };
    expect(unwrapEnvelope(body)).toBe(body);
  });

  it("falls back to the body when data is nullish (??, not ||)", () => {
    const body = { data: null, extra: 1 };
    // null is the one falsy value ?? catches — the body comes back whole.
    expect(unwrapEnvelope(body)).toBe(body);
    const body2 = { data: undefined };
    expect(unwrapEnvelope(body2)).toBe(body2);
  });

  it("preserves a falsy-but-present data value (0, '', false)", () => {
    // ?? only catches null/undefined, so these survive — matching the prior
    // inline `body.data ?? body` behaviour exactly.
    expect(unwrapEnvelope({ data: 0 })).toBe(0);
    expect(unwrapEnvelope({ data: "" })).toBe("");
    expect(unwrapEnvelope({ data: false })).toBe(false);
  });

  it("passes a non-object body through unchanged", () => {
    expect(unwrapEnvelope(null)).toBeNull();
    expect(unwrapEnvelope("hello")).toBe("hello");
    expect(unwrapEnvelope(42)).toBe(42);
  });
});

describe("decodeSearchHit", () => {
  it("rebuilds a valid hit field-by-field (not the same reference)", () => {
    const hit = decodeSearchHit({ ...VALID_HIT });
    expect(hit).toEqual(VALID_HIT);
    expect(hit).not.toBe(VALID_HIT);
  });

  it("defaults a missing/non-string key to empty string", () => {
    expect(decodeSearchHit({ surah: 1, ayah: 1, globalIndex: 0, text: "" })?.key).toBe("");
    expect(decodeSearchHit({ key: 99, surah: 1, ayah: 1 })?.key).toBe("");
  });

  it("defaults a missing/non-string text to empty string", () => {
    expect(decodeSearchHit({ surah: 1, ayah: 1 })?.text).toBe("");
    expect(decodeSearchHit({ text: 123, surah: 1, ayah: 1 })?.text).toBe("");
  });

  it("coerces a numeric globalIndex and defaults to 0 when absent", () => {
    expect(decodeSearchHit({ surah: 1, ayah: 1, globalIndex: 42 })?.globalIndex).toBe(42);
    expect(decodeSearchHit({ surah: 1, ayah: 1 })?.globalIndex).toBe(0);
  });

  it.each([
    ["surah is a string", { surah: "2", ayah: 1 }],
    ["ayah is a string", { surah: 2, ayah: "1" }],
    ["surah is 0 (out of range)", { surah: 0, ayah: 1 }],
    ["ayah is 0 (out of range)", { surah: 2, ayah: 0 }],
    ["surah is negative", { surah: -1, ayah: 1 }],
    ["surah is NaN", { surah: NaN, ayah: 1 }],
    ["ayah is NaN", { surah: 2, ayah: NaN }],
    ["surah is Infinity", { surah: Infinity, ayah: 1 }],
    ["surah is true (would coerce to 1 via Number())", { surah: true, ayah: 1 }],
    ["surah is null", { surah: null, ayah: 1 }],
    ["surah is missing", { ayah: 1 }],
  ])("rejects the hit when %s", (_label, rec) => {
    expect(decodeSearchHit(rec)).toBeNull();
  });

  it.each([[null], [undefined], ["string"], [42], [true]])(
    "rejects a non-object input (%s)",
    (input) => {
      expect(decodeSearchHit(input)).toBeNull();
    },
  );
});

describe("decodeSearchResponse", () => {
  const makePayload = (overrides: Record<string, unknown> = {}) => ({
    query: "allah",
    total: 1,
    limit: 20,
    offset: 0,
    results: [VALID_HIT],
    ...overrides,
  });

  it("decodes a full valid payload, rebuilding every hit", () => {
    const decoded = decodeSearchResponse(makePayload());
    expect(decoded).toEqual({
      query: "allah",
      total: 1,
      limit: 20,
      offset: 0,
      results: [VALID_HIT],
    });
  });

  it("decodes with only `results` present (scalars become null)", () => {
    const decoded = decodeSearchResponse({ results: [VALID_HIT] });
    expect(decoded).toEqual({
      query: null,
      total: null,
      limit: null,
      offset: null,
      results: [VALID_HIT],
    });
  });

  it("drops malformed hits rather than failing the whole payload", () => {
    const decoded = decodeSearchResponse({
      results: [VALID_HIT, { surah: "x", ayah: 1 }, null, { surah: 3, ayah: 1 }],
    });
    expect(decoded?.results).toEqual([
      VALID_HIT,
      { key: "", surah: 3, ayah: 1, globalIndex: 0, text: "" },
    ]);
  });

  it("returns null when results is missing or not an array", () => {
    expect(decodeSearchResponse({ total: 1 })).toBeNull();
    expect(decodeSearchResponse({ results: "nope" })).toBeNull();
    expect(decodeSearchResponse({ results: null })).toBeNull();
  });

  it.each([[null], [undefined], ["x"], [42]])("returns null for non-object input (%s)", (input) => {
    expect(decodeSearchResponse(input)).toBeNull();
  });

  it("surfaces non-finite scalars as null (NaN, Infinity)", () => {
    const decoded = decodeSearchResponse({ results: [], total: NaN, limit: Infinity });
    expect(decoded).toEqual({ query: null, total: null, limit: null, offset: null, results: [] });
  });

  it("does NOT strip the envelope itself — the caller owns that", () => {
    // An enveloped body handed straight to decodeSearchResponse has no top-level
    // `results`, so it is rejected. The composition with unwrapEnvelope works.
    const enveloped = { data: makePayload() };
    expect(decodeSearchResponse(enveloped)).toBeNull();
    expect(decodeSearchResponse(unwrapEnvelope(enveloped))).toEqual({
      query: "allah",
      total: 1,
      limit: 20,
      offset: 0,
      results: [VALID_HIT],
    });
  });
});

describe("decodeScript", () => {
  const uthmani = {
    id: "uthmani",
    sizeBytes: 1234,
    sha256: "abc",
    downloadUrl: "https://cdn/uthmani.db",
  };

  it("decodes a valid uthmani spec", () => {
    expect(decodeScript(uthmani)).toEqual(uthmani);
  });

  it("decodes a valid simple-clean spec", () => {
    const spec = { ...uthmani, id: "simple-clean" };
    expect(decodeScript(spec)).toEqual(spec);
  });

  it.each([
    ["unknown id", { ...uthmani, id: "hafs" }],
    ["id missing", { sizeBytes: 1, sha256: "a", downloadUrl: "u" }],
    ["sizeBytes zero (falsy)", { ...uthmani, sizeBytes: 0 }],
    ["sizeBytes missing", { id: "uthmani", sha256: "a", downloadUrl: "u" }],
    ["sha256 non-string", { ...uthmani, sha256: 123 }],
    ["sha256 missing", { id: "uthmani", sizeBytes: 1, downloadUrl: "u" }],
    ["downloadUrl non-string", { ...uthmani, downloadUrl: null }],
    ["downloadUrl missing", { id: "uthmani", sizeBytes: 1, sha256: "a" }],
  ])("rejects when %s", (_label, rec) => {
    expect(decodeScript(rec)).toBeNull();
  });

  it.each([[null], [undefined], ["x"], [42]])("rejects non-object input (%s)", (input) => {
    expect(decodeScript(input)).toBeNull();
  });
});

describe("decodeScriptsPayload", () => {
  const scripts = [
    { id: "uthmani", sizeBytes: 1, sha256: "a", downloadUrl: "https://x/u" },
    { id: "simple-clean", sizeBytes: 2, sha256: "b", downloadUrl: "https://x/s" },
  ];

  it("decodes a bare { scripts: [...] } body", () => {
    expect(decodeScriptsPayload({ scripts })).toHaveLength(2);
  });

  it("strips a { data: { scripts } } envelope", () => {
    expect(decodeScriptsPayload({ data: { scripts } })).toHaveLength(2);
  });

  it("drops invalid entries and keeps the valid ones", () => {
    const out = decodeScriptsPayload({
      scripts: [...scripts, { id: "hafs", sizeBytes: 1, sha256: "c", downloadUrl: "u" }, null],
    });
    expect(out).toHaveLength(2);
    expect(out?.map((s) => s.id).sort()).toEqual(["simple-clean", "uthmani"]);
  });

  it("returns null when scripts is missing or not an array", () => {
    expect(decodeScriptsPayload({})).toBeNull();
    expect(decodeScriptsPayload({ scripts: "nope" })).toBeNull();
    expect(decodeScriptsPayload({ data: { scripts: 42 } })).toBeNull();
  });

  it("returns null for a non-object body (including after envelope unwrap)", () => {
    expect(decodeScriptsPayload(null)).toBeNull();
    expect(decodeScriptsPayload("hello")).toBeNull();
    expect(decodeScriptsPayload({ data: null })).toBeNull();
  });
});

describe("decodeVersionPayload", () => {
  it("decodes a bare { contentVersion, searchVersion } body", () => {
    expect(decodeVersionPayload({ contentVersion: "cv1", searchVersion: "sv1" })).toEqual({
      contentVersion: "cv1",
      searchVersion: "sv1",
    });
  });

  it("strips a { data: { ... } } envelope", () => {
    expect(decodeVersionPayload({ data: { contentVersion: "cv1", searchVersion: "sv1" } })).toEqual(
      {
        contentVersion: "cv1",
        searchVersion: "sv1",
      },
    );
  });

  it("surfaces missing/non-string fields as null (caller applies baked defaults)", () => {
    expect(decodeVersionPayload({ contentVersion: "cv1" })).toEqual({
      contentVersion: "cv1",
      searchVersion: null,
    });
    expect(decodeVersionPayload({ contentVersion: 99, searchVersion: false })).toEqual({
      contentVersion: null,
      searchVersion: null,
    });
  });

  it("returns null for a non-object body", () => {
    expect(decodeVersionPayload(null)).toBeNull();
    expect(decodeVersionPayload("x")).toBeNull();
    // A non-object `data` (e.g. a number) surfaces after envelope unwrap and
    // is rejected. `{ data: null }` is NOT rejected here: null is nullish, so
    // unwrapEnvelope falls back to the body, which then yields all-null fields.
    expect(decodeVersionPayload({ data: 42 })).toBeNull();
  });
});
