/* eslint-disable @typescript-eslint/unbound-method */
import { flushSync } from "svelte";
import { describe, it, expect, vi } from "vite-plus/test";

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: "local",
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import { createReader, type ReaderApi } from "../reader.svelte";
import { observeVersesFor } from "./reader-reactivity.probe.svelte";

type RemovedReaderMembers = {
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

  it("openNote toggles; markRead moves the tab's reading thread", () => {
    const r = createReader();
    expect(r.openNote).toBeNull();
    r.toggleNote("1:1");
    expect(r.openNote).toBe("1:1");
    r.toggleNote("1:1");
    expect(r.openNote).toBeNull();
    r.markRead(2, 256, "ms.basmeih");
    expect(r.hasLastRead).toBe(true);
    expect(r.lastRead).toEqual({ num: 2, n: 256, sourceId: "ms.basmeih" });
    r.markRead(2, 257);
    expect(r.lastRead).toEqual({ num: 2, n: 257 });
  });

  it("setLastReadAnchor rides the thread; a cursor move clears it; pendingAnchor round-trips", () => {
    const r = createReader();
    r.markRead(2, 255);
    r.setLastReadAnchor({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    expect(r.lastReadAnchor).toEqual({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    r.markRead(2, 256);
    expect(r.lastReadAnchor).toBeNull();
    r.setPendingAnchor({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.pendingAnchor).toEqual({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.consumePendingAnchor()).toEqual({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(r.pendingAnchor).toBeNull();
  });

  it("two createReader() instances are isolated", () => {
    const a = createReader();
    const b = createReader();
    a.setNote("1:1", "in-a");
    expect(b.getNote("1:1")).toBe("");
  });
});
