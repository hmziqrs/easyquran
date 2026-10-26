import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/env", () => ({
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

  const read = (): { v: number; ids: string[]; enabled?: boolean } | null =>
    JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("starts empty and persists nothing until written", () => {
    expect(store.ids).toEqual([]);
    expect(read()).toBeNull();
  });

  it("setIds persists {v:1,ids} and is readable by a fresh instance", () => {
    store.setIds(["en.sahih", "ur.jalandhry"]);
    expect(read()).toEqual({ v: 1, ids: ["en.sahih", "ur.jalandhry"], enabled: true });
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
    expect(read()).toEqual({ v: 1, ids: [], enabled: true });
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

  it("starts enabled with activeIds mirroring ids", () => {
    store.setIds(["en.sahih"]);
    expect(store.enabled).toBe(true);
    expect(store.activeIds).toEqual(["en.sahih"]);
  });

  it("setEnabled(false) pauses the flow but keeps every selection", () => {
    store.setIds(["en.sahih", "ur.jalandhry"]);
    store.setEnabled(false);
    expect(store.enabled).toBe(false);
    expect(store.ids).toEqual(["en.sahih", "ur.jalandhry"]);
    expect(store.activeIds).toEqual([]);
    expect(read()).toEqual({
      v: 1,
      ids: ["en.sahih", "ur.jalandhry"],
      enabled: false,
    });
  });

  it("toggleEnabled flips the pause back on", () => {
    store.setIds(["en.sahih"]);
    store.toggleEnabled();
    expect(store.activeIds).toEqual([]);
    store.toggleEnabled();
    expect(store.activeIds).toEqual(["en.sahih"]);
  });

  it("a paused store keeps curating: reorder/remove persist while activeIds stays empty", () => {
    store.setIds(["a", "b"]);
    store.setEnabled(false);
    store.reorder("b", -1);
    store.remove("a");
    expect(store.ids).toEqual(["b"]);
    expect(store.activeIds).toEqual([]);
  });

  it("a fresh instance restores the paused state and its selections", () => {
    store.setIds(["en.sahih"]);
    store.setEnabled(false);
    const fresh = new StackedTranslationsStore();
    expect(fresh.enabled).toBe(false);
    expect(fresh.ids).toEqual(["en.sahih"]);
    expect(fresh.activeIds).toEqual([]);
    fresh.dispose();
  });

  it("a v1 blob without the enabled field reads as enabled", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, ids: ["en.sahih"] }));
    const fresh = new StackedTranslationsStore();
    expect(fresh.enabled).toBe(true);
    expect(fresh.activeIds).toEqual(["en.sahih"]);
    fresh.dispose();
  });

  it("reconciles an enabled flip from a foreign-tab storage event", () => {
    store.setIds(["en.sahih"]);
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, ids: ["en.sahih"], enabled: false }));
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(store.enabled).toBe(false);
    expect(store.activeIds).toEqual([]);
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
