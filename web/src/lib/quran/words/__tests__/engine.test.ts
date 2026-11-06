import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { QuranScript } from "#lib/data/quran-types.js";
import { clearWordTables, loadWordTable, lookupWord } from "#lib/quran/words/engine.js";
import { wordSourceFor } from "#lib/quran/words/gate.js";
import { validateWordSources, WORD_SOURCES } from "#lib/quran/words/registry.js";
import { wordKey, type WordEntry, type WordKey, type WordSource } from "#lib/quran/words/types.js";

// Fake source for tests only. Registry stays empty in production.
const FAKE_ENTRY: WordEntry = { meaning: "in", transliteration: "bi" };

function fakeTable(surah: number): ReadonlyMap<WordKey, WordEntry> {
  return new Map<WordKey, WordEntry>([[wordKey({ surah, ayah: 1, word: 1 }), FAKE_ENTRY]]);
}

function fakeSource(load: WordSource["load"] = async (surah) => fakeTable(surah)): WordSource {
  return {
    id: "test.fake",
    meaningLanguage: "en",
    meaningDirection: "ltr",
    licence: "CC0-1.0",
    attribution: "Fake fixture",
    delivery: "bundled",
    load,
  };
}

beforeEach(() => {
  clearWordTables();
});

describe("word registry (empty until O23)", () => {
  it("ships no word source, so the reader path loads no word code", () => {
    expect(WORD_SOURCES).toHaveLength(0);
    expect(wordSourceFor(QuranScript.Uthmani, WORD_SOURCES)).toBeUndefined();
  });

  it("validates credit, licence and id on every source", () => {
    expect(() => validateWordSources([fakeSource()])).not.toThrow();
    expect(() => validateWordSources([{ ...fakeSource(), licence: " " }])).toThrow(/no licence/);
    expect(() => validateWordSources([{ ...fakeSource(), attribution: "" }])).toThrow(
      /no attribution/,
    );
    expect(() => validateWordSources([fakeSource(), fakeSource()])).toThrow(/duplicate/);
  });
});

describe("wordSourceFor gate", () => {
  const sources = [fakeSource()];

  it("serves the plain Arabic scripts only", () => {
    expect(wordSourceFor(QuranScript.Uthmani, sources)?.id).toBe("test.fake");
    expect(wordSourceFor(QuranScript.SimpleClean, sources)?.id).toBe("test.fake");
    expect(wordSourceFor(QuranScript.Tajweed, sources)).toBeUndefined();
    expect(wordSourceFor(QuranScript.IndoPak, sources)).toBeUndefined();
  });
});

describe("word engine", () => {
  it("looks up by canonical coordinate and returns undefined for a gap", () => {
    const table = fakeTable(1);
    expect(lookupWord(table, { surah: 1, ayah: 1, word: 1 })).toEqual(FAKE_ENTRY);
    expect(lookupWord(table, { surah: 1, ayah: 1, word: 2 })).toBeUndefined();
    expect(lookupWord(table, { surah: 1, ayah: 2, word: 1 })).toBeUndefined();
  });

  it("loads one table per surah and shares it", async () => {
    const load = vi.fn<WordSource["load"]>(async (surah) => fakeTable(surah));
    const source = fakeSource(load);
    const [first, second] = await Promise.all([loadWordTable(source, 1), loadWordTable(source, 1)]);
    expect(first).toBe(second);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("drops a failed load so the next open retries", async () => {
    const load = vi
      .fn<WordSource["load"]>()
      .mockRejectedValueOnce(new Error("network"))
      .mockImplementation(async (surah) => fakeTable(surah));
    const source = fakeSource(load);
    await expect(loadWordTable(source, 1)).rejects.toThrow("network");
    const table = await loadWordTable(source, 1);
    expect(lookupWord(table, { surah: 1, ayah: 1, word: 1 })).toEqual(FAKE_ENTRY);
    expect(load).toHaveBeenCalledTimes(2);
  });
});
