import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/environment", () => ({
  get browser() {
    return flag.value;
  },
}));

import { StackedTranslationsStore, STACKED_MAX_EXTRAS } from "../stacked-translations.svelte";

const KEY = "easyquran.reader.stacked";

describe("StackedTranslationsStore", () => {
  let store: StackedTranslationsStore;

  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
    store = new StackedTranslationsStore();
  });
  afterEach(() => store.dispose());

  const read = (): { v: number; ids: string[] } | null =>
    JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("starts empty and persists nothing until written", () => {
    expect(store.ids).toEqual([]);
    expect(read()).toBeNull();
  });

  it("setIds persists {v:1,ids} and is readable by a fresh instance", () => {
    store.setIds(["en.sahih", "ur.jalandhry"]);
    expect(read()).toEqual({ v: 1, ids: ["en.sahih", "ur.jalandhry"] });
    const fresh = new StackedTranslationsStore();
    expect(fresh.ids).toEqual(["en.sahih", "ur.jalandhry"]);
    fresh.dispose();
  });

  it("setIds dedupes preserving first occurrence", () => {
    store.setIds(["en.sahih", "en.sahih", "ur.jalandhry"]);
    expect(store.ids).toEqual(["en.sahih", "ur.jalandhry"]);
  });

  it("setIds caps at STACKED_MAX_EXTRAS", () => {
    store.setIds(["a", "b", "c", "d", "e", "f"]);
    expect(store.ids).toEqual(["a", "b", "c", "d", "e"]);
    expect(store.ids.length).toBe(STACKED_MAX_EXTRAS);
  });

  it("toggle adds then removes", () => {
    store.toggle("en.sahih");
    expect(store.ids).toEqual(["en.sahih"]);
    store.toggle("en.sahih");
    expect(store.ids).toEqual([]);
  });

  it("toggle beyond the cap is ignored", () => {
    store.setIds(["a", "b", "c", "d", "e"]);
    store.toggle("f");
    expect(store.ids).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("reorder moves an id up and down", () => {
    store.setIds(["a", "b", "c"]);
    store.reorder("b", -1);
    expect(store.ids).toEqual(["b", "a", "c"]);
    store.reorder("a", 1);
    expect(store.ids).toEqual(["b", "c", "a"]);
  });

  it("reorder ignores out-of-range and unknown ids", () => {
    store.setIds(["a", "b"]);
    store.reorder("a", -1);
    expect(store.ids).toEqual(["a", "b"]);
    store.reorder("b", 1);
    expect(store.ids).toEqual(["a", "b"]);
    store.reorder("z", 1);
    expect(store.ids).toEqual(["a", "b"]);
  });

  it("remove drops an id", () => {
    store.setIds(["a", "b", "c"]);
    store.remove("b");
    expect(store.ids).toEqual(["a", "c"]);
  });

  it("clear empties the store and persists the empty shape", () => {
    store.setIds(["a", "b"]);
    store.clear();
    expect(store.ids).toEqual([]);
    expect(read()).toEqual({ v: 1, ids: [] });
  });

  it("decodes a future-schema blob to []", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ v: 99, ids: ["en.sahih"] }));
    const fresh = new StackedTranslationsStore();
    expect(fresh.ids).toEqual([]);
    fresh.dispose();
  });

  it("decodes a non-array ids field to []", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, ids: "nope" }));
    const fresh = new StackedTranslationsStore();
    expect(fresh.ids).toEqual([]);
    fresh.dispose();
  });

  it("filters non-string junk elements during decode", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ v: 1, ids: ["en.sahih", 42, null, "ur.jalandhry"] }),
    );
    const fresh = new StackedTranslationsStore();
    expect(fresh.ids).toEqual(["en.sahih", "ur.jalandhry"]);
    fresh.dispose();
  });

  it("decodes a versionless legacy blob tolerantly", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ ids: ["en.sahih"] }));
    const fresh = new StackedTranslationsStore();
    expect(fresh.ids).toEqual(["en.sahih"]);
    fresh.dispose();
  });

  it("reconciles from a foreign-tab storage event", () => {
    store.setIds(["en.sahih"]);
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, ids: ["ur.jalandhry", "fr.hamid"] }));
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(store.ids).toEqual(["ur.jalandhry", "fr.hamid"]);
  });

  it("ignores storage events for other keys", () => {
    store.setIds(["en.sahih"]);
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.prefs" }));
    expect(store.ids).toEqual(["en.sahih"]);
  });

  it("dispose detaches the cross-tab listener", () => {
    store.setIds(["en.sahih"]);
    store.dispose();
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, ids: ["ur.jalandhry"] }));
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(store.ids).toEqual(["en.sahih"]);
  });
});
