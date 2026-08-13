/* eslint-disable @typescript-eslint/unbound-method */
import { flushSync } from "svelte";
import { describe, it, expect } from "vite-plus/test";

import { createReader, type ReaderApi } from "../reader.svelte";
import { observeVersesFor } from "./reader-reactivity.probe.svelte";

type RemovedReaderMembers = {
  current?: undefined;
  surah?: undefined;
  surahCount?: undefined;
  fontSize?: undefined;
  bookmarkList?: undefined;
  bookmarkCount?: undefined;
};

describe("createReader — preserved public API", () => {
  const r: ReaderApi = createReader();

  it("exposes every USED member consumers depend on", () => {
    expect(r.hydrate).toBeTypeOf("function");
    expect(r.setQuery).toBeTypeOf("function");
    expect(r.clearQuery).toBeTypeOf("function");
    expect(r.setBrowse).toBeTypeOf("function");
    expect(r.toggleNote).toBeTypeOf("function");
    expect(r.setCurrent).toBeTypeOf("function");
    expect(r.openVerse).toBeTypeOf("function");
    expect(r.markRead).toBeTypeOf("function");
    expect(r.bigger).toBeTypeOf("function");
    expect(r.smaller).toBeTypeOf("function");
    expect(r.setMode).toBeTypeOf("function");
    expect(r.isBookmarked).toBeTypeOf("function");
    expect(r.toggleBookmark).toBeTypeOf("function");
    expect(r.getNote).toBeTypeOf("function");
    expect(r.setNote).toBeTypeOf("function");
    expect(r.versesFor).toBeTypeOf("function");
    expect(r.seedAyahs).toBeTypeOf("function");
    expect(r.refreshFromWorker).toBeTypeOf("function");
    expect(r.copyVerse).toBeTypeOf("function");
    expect(r.shareVerse).toBeTypeOf("function");
  });

  it("removed the verified-dead public members", () => {
    // SAFETY: createReader() returns the closed ReaderApi contract; the probed members are the verified-removed set, so each lookup is undefined.
    const dead = r as RemovedReaderMembers;
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

  it("seedAyahs/versesFor preserve explicit verse coordinates", () => {
    const r = createReader();
    r.seedAyahs([
      { key: "1:1", text: "bismillah" },
      { key: "1:2", text: "verse2" },
    ]);
    expect(r.versesFor(1)).toEqual(["bismillah", "verse2"]);
  });

  it("SvelteMap verse cache is reactive (regression: plain Map silently failed)", () => {
    const r = createReader();
    const obs = observeVersesFor(r, 2);
    flushSync();
    expect(obs.runs()).toBe(1);
    expect(obs.latest()).toEqual([]);
    r.seedAyahs([
      { key: "2:1", text: "a" },
      { key: "2:2", text: "b" },
    ]);
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
    r.markRead(2, 256);
    expect(r.lastRead).toEqual({ num: 2, n: 256 });
  });

  it("markRead / openVerse thread sourceId for cross-surface resume", () => {
    const r = createReader();
    r.markRead(31, 5, "ms.basmeih");
    expect(r.lastRead).toEqual({ num: 31, n: 5, sourceId: "ms.basmeih" });
    r.openVerse(31, 5, "ms.basmeih");
    expect(r.lastRead).toEqual({ num: 31, n: 5, sourceId: "ms.basmeih" });
    r.markRead(31, 5);
    expect(r.lastRead).toEqual({ num: 31, n: 5 });
  });

  it("setLastReadAnchor / pendingAnchor / openVerse clears the anchor", () => {
    const r = createReader();
    r.setLastReadAnchor({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    expect(r.lastReadAnchor).toEqual({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    r.openVerse(2, 255);
    expect(r.lastReadAnchor).toBeNull();
    r.setPendingAnchor({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.pendingAnchor).toEqual({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.consumePendingAnchor()).toEqual({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.pendingAnchor).toBeNull();
  });

  it("openVerse/markRead push per-surah recents (dedup, move-to-front, clear)", () => {
    const r = createReader();
    r.openVerse(2, 5);
    r.openVerse(2, 255);
    r.openVerse(36, 1);
    expect(r.recentReads[0]).toEqual({ num: 36, n: 1, ts: expect.any(Number) });
    expect(r.recentReads.find((x) => x.num === 2)?.n).toBe(255);
    expect(r.recentReads.length).toBe(2);
    r.clearReadingPosition();
    expect(r.recentReads).toEqual([]);
  });

  it("openVerse caps recents at the limit", () => {
    const r = createReader();
    for (let i = 1; i <= 10; i++) r.openVerse(i, 1);
    expect(r.recentReads.length).toBeLessThanOrEqual(8);
    expect(r.recentReads[0]?.num).toBe(10);
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
