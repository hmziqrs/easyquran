import { describe, expect, it } from "vite-plus/test";
import { reader } from "$lib/stores/reader.svelte";
import { track } from "$lib/test/reactive.svelte";

/**
 * Surah numbers 800-802 do not exist in the catalog, so they are never touched
 * by real navigation — a clean scratchpad for the sync verse cache that avoids
 * cross-test interference on the shared `reader` singleton.
 */
const SCRATCH_UNSEEDED = 800;
const SCRATCH_SEEDED = 801;
const SCRATCH_REACTIVE = 802;

describe("reader synchronous verse cache", () => {
  it("returns [] for a surah that has not been seeded this session", () => {
    expect(reader.versesFor(SCRATCH_UNSEEDED)).toEqual([]);
  });

  it("seedSurah stores verses that versesFor then returns", () => {
    reader.seedSurah(SCRATCH_SEEDED, ["one", "two"]);
    expect(reader.versesFor(SCRATCH_SEEDED)).toEqual(["one", "two"]);
  });

  it("does not seed an empty verse array", () => {
    reader.seedSurah(SCRATCH_UNSEEDED, []);
    expect(reader.versesFor(SCRATCH_UNSEEDED)).toEqual([]);
  });

  it("reactively notifies consumers when a surah is seeded", async () => {
    // Regression for the non-reactive Map: with `#versesBySurah = $state(new Map())`,
    // `.get()` is NOT tracked against `.set()`, so `seen.runs` stays at 1 after
    // seedSurah and the Sidebar ayah list never populates without an unrelated
    // state update. SvelteMap makes the get()/.set() pair reactive.
    const seen = await track(() => reader.versesFor(SCRATCH_REACTIVE));

    expect(seen.runs).toBe(1);
    expect(seen.value).toEqual([]);

    reader.seedSurah(SCRATCH_REACTIVE, ["foo", "bar"]);
    await seen.settle();

    expect(seen.runs).toBe(2);
    expect(seen.value).toEqual(["foo", "bar"]);

    seen.dispose();
  });
});
