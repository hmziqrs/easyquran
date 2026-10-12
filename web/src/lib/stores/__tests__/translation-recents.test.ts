import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));

import { TRANSLATION_RECENTS_MAX, TranslationRecentsStore } from "../translation-recents.svelte";

const KEY = "easyquran.translations.recent";

describe("TranslationRecentsStore", () => {
  let store: TranslationRecentsStore;

  beforeEach(() => {
    window.localStorage.clear();
    store = new TranslationRecentsStore();
  });
  afterEach(() => store.dispose());

  // SAFETY: readJSON parses whatever is under the key; tests only ever write valid envelopes.
  const read = (): {
    v: number;
    entries: { id: string; lastUsedAt: number; useCount: number }[];
  } | null => JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("record persists the v1 envelope and a fresh instance reads it back", () => {
    store.record("en.sahih", 1000);
    expect(read()).toEqual({
      v: 1,
      entries: [{ id: "en.sahih", lastUsedAt: 1000, useCount: 1 }],
    });
    const fresh = new TranslationRecentsStore();
    expect([...fresh.ids]).toEqual(["en.sahih"]);
    fresh.dispose();
  });

  it("record moves an existing id to the front and bumps its useCount", () => {
    store.record("en.sahih", 1000);
    store.record("ur.jalandhry", 2000);
    store.record("en.sahih", 3000);
    expect([...store.ids]).toEqual(["en.sahih", "ur.jalandhry"]);
    expect(read()?.entries[0]).toEqual({ id: "en.sahih", lastUsedAt: 3000, useCount: 2 });
  });

  it("caps the MRU at TRANSLATION_RECENTS_MAX, dropping the oldest", () => {
    for (let i = 0; i < TRANSLATION_RECENTS_MAX + 3; i += 1) store.record(`t.${i}`, i);
    expect(store.ids).toHaveLength(TRANSLATION_RECENTS_MAX);
    // most recent first; the first-recorded ids fell off the tail
    expect(store.ids[0]).toBe(`t.${TRANSLATION_RECENTS_MAX + 2}`);
    expect(store.rankOf("t.0")).toBe(-1);
    expect(store.rankOf("t.2")).toBe(-1);
  });

  it("rankIds orders known ids most-recently-used first and keeps unknown order after", () => {
    store.record("en.sahih", 1000);
    store.record("ur.jalandhry", 2000);
    store.record("fr.hamidullah", 3000);
    expect(store.rankIds(["en.sahih", "fr.hamidullah", "ur.jalandhry"])).toEqual([
      "fr.hamidullah",
      "ur.jalandhry",
      "en.sahih",
    ]);
    // unknown ids keep their given order after every ranked one
    expect(store.rankIds(["zz.unknown", "en.sahih", "aa.unknown"])).toEqual([
      "en.sahih",
      "zz.unknown",
      "aa.unknown",
    ]);
  });

  it("decodes drop malformed entries and duplicates, and cap the list", () => {
    const entries = [
      { id: "en.sahih", lastUsedAt: 1, useCount: 1 },
      { id: "en.sahih", lastUsedAt: 2, useCount: 5 },
      { id: "", lastUsedAt: 3, useCount: 1 },
      { lastUsedAt: 4, useCount: 1 },
      { id: "ur.jalandhry", lastUsedAt: "soon", useCount: 1 },
      { id: "fr.hamidullah", lastUsedAt: 5, useCount: 2 },
    ];
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, entries }));
    const fresh = new TranslationRecentsStore();
    expect([...fresh.ids]).toEqual(["en.sahih", "fr.hamidullah"]);
    fresh.dispose();
  });

  it("keeps the per-author last-used variant after the MRU evicts it (Feature 1.4)", () => {
    const author = "en saheeh international";
    store.record("en.sahih", 1000, author);
    store.record("qul.r20.en-sahih-international", 2000, author);
    // flood the MRU past the cap: the author's entries fall off the tail
    for (let i = 0; i < TRANSLATION_RECENTS_MAX; i += 1) store.record(`t.${i}`, 3000 + i);
    expect(store.rankOf("en.sahih")).toBe(-1);
    expect(store.rankOf("qul.r20.en-sahih-international")).toBe(-1);
    expect(store.authorVariant(author)).toBe("qul.r20.en-sahih-international");
    // persisted: a fresh instance reads the variant memory back
    const fresh = new TranslationRecentsStore();
    expect(fresh.authorVariant(author)).toBe("qul.r20.en-sahih-international");
    fresh.dispose();
  });

  it("a future schema resets to empty instead of guessing", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ v: 2, entries: [{ id: "en.sahih" }] }));
    const fresh = new TranslationRecentsStore();
    expect(fresh.ids).toEqual([]);
    fresh.dispose();
  });
});
