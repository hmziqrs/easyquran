import { describe, it, expect } from "vite-plus/test";
import { flushSync } from "svelte";
import { createReader, type ReaderApi } from "../reader.svelte";
import { observeVersesFor } from "./reader-reactivity.probe.svelte";

describe("createReader — preserved public API", () => {
  const r: ReaderApi = createReader();

  it("exposes every USED member consumers depend on", () => {
    expect(typeof r.hydrate).toBe("function");
    expect(typeof r.setQuery).toBe("function");
    expect(typeof r.clearQuery).toBe("function");
    expect(typeof r.setBrowse).toBe("function");
    expect(typeof r.toggleNote).toBe("function");
    expect(typeof r.setCurrent).toBe("function");
    expect(typeof r.openVerse).toBe("function");
    expect(typeof r.bigger).toBe("function");
    expect(typeof r.smaller).toBe("function");
    expect(typeof r.setMode).toBe("function");
    expect(typeof r.isBookmarked).toBe("function");
    expect(typeof r.toggleBookmark).toBe("function");
    expect(typeof r.getNote).toBe("function");
    expect(typeof r.setNote).toBe("function");
    expect(typeof r.versesFor).toBe("function");
    expect(typeof r.seedSurah).toBe("function");
    expect(typeof r.refreshFromWorker).toBe("function");
    expect(typeof r.copyVerse).toBe("function");
    expect(typeof r.shareVerse).toBe("function");
  });

  it("removed the verified-dead public members", () => {
    const dead = r as unknown as Record<string, unknown>;
    expect(dead.current).toBeUndefined();
    expect(dead.surah).toBeUndefined();
    expect(dead.surahCount).toBeUndefined();
    expect(dead.fontSize).toBeUndefined();
    expect(dead.bookmarkList).toBeUndefined();
    expect(dead.bookmarkCount).toBeUndefined();
  });
});

describe("createReader — behaviour", () => {
  it("defaults match the expected starting state", () => {
    const r = createReader();
    expect(r.query).toBe("");
    expect(r.hasQuery).toBe(false);
    expect(r.browseMode).toBe("surah");
    expect(r.mode).toBe("verse");
    expect(r.isVerseMode).toBe(true);
    expect(r.isReadingMode).toBe(false);
    expect(r.arabicSizePx).toBe("33px");
    expect(r.hasLastRead).toBe(false);
    expect(r.lastReadRef).toBe("");
    expect(r.versesFor(1)).toEqual([]);
  });

  it("seedSurah/versesFor read back seeded verses", () => {
    const r = createReader();
    r.seedSurah(1, ["bismillah", "verse2"]);
    expect(r.versesFor(1)).toEqual(["bismillah", "verse2"]);
  });

  it("SvelteMap verse cache is reactive (regression: plain Map silently failed)", () => {
    const r = createReader();
    const obs = observeVersesFor(r, 2);
    flushSync();
    expect(obs.runs()).toBe(1);
    expect(obs.latest()).toEqual([]);
    r.seedSurah(2, ["a", "b"]);
    flushSync();
    expect(obs.runs()).toBeGreaterThan(1);
    expect(obs.latest()).toEqual(["a", "b"]);
    obs.dispose();
  });

  it("bookmarks toggle and persist-reactive", () => {
    const r = createReader();
    expect(r.isBookmarked("1:1")).toBe(false);
    r.toggleBookmark("1:1");
    expect(r.isBookmarked("1:1")).toBe(true);
    r.toggleBookmark("1:1");
    expect(r.isBookmarked("1:1")).toBe(false);
  });

  it("notes set/get round-trip", () => {
    const r = createReader();
    expect(r.getNote("2:2")).toBe("");
    r.setNote("2:2", "my note");
    expect(r.getNote("2:2")).toBe("my note");
  });

  it("font size respects bounds and exposes arabicSizePx", () => {
    const r = createReader();
    for (let i = 0; i < 100; i++) r.bigger();
    expect(r.arabicSizePx).toBe("56px");
    for (let i = 0; i < 100; i++) r.smaller();
    expect(r.arabicSizePx).toBe("22px");
  });

  it("mode setters and browse setters update state", () => {
    const r = createReader();
    r.setMode("reading");
    expect(r.mode).toBe("reading");
    expect(r.isReadingMode).toBe(true);
    r.setMode("reading");
    expect(r.mode).toBe("reading");
    r.setBrowse("juz");
    expect(r.browseJuz).toBe(true);
    expect(r.browseMode).toBe("juz");
  });

  it("query mutators update hasQuery", () => {
    const r = createReader();
    r.setQuery("baqarah");
    expect(r.hasQuery).toBe(true);
    r.clearQuery();
    expect(r.hasQuery).toBe(false);
  });

  it("openNote toggles and lastRead updates on openVerse", () => {
    const r = createReader();
    expect(r.openNote).toBeNull();
    r.toggleNote("1:1");
    expect(r.openNote).toBe("1:1");
    r.toggleNote("1:1");
    expect(r.openNote).toBeNull();
    r.openVerse(2, 255);
    expect(r.hasLastRead).toBe(true);
    expect(r.lastRead).toEqual({ num: 2, n: 255 });
  });

  it("two createReader() instances are isolated", () => {
    const a = createReader();
    const b = createReader();
    a.setNote("1:1", "in-a");
    expect(b.getNote("1:1")).toBe("");
    a.toggleBookmark("5:1");
    expect(b.isBookmarked("5:1")).toBe(false);
  });
});
