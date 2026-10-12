import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/env", () => ({
  get browser() {
    return flag.value;
  },
}));

import { SEARCH_RECENTS_MAX, SearchRecentsStore } from "../search-recents.svelte";

const KEY = "easyquran.search.recent";

describe("SearchRecentsStore", () => {
  let store: SearchRecentsStore;

  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
    store = new SearchRecentsStore();
  });
  afterEach(() => store.dispose());

  const read = (): unknown => JSON.parse(window.localStorage.getItem(KEY) ?? "null");
  const seed = (entries: { q: string; at: number }[]): void => {
    window.localStorage.setItem(KEY, JSON.stringify({ v: 1, entries, paused: false }));
  };

  it("starts empty and persists nothing until a search runs", () => {
    expect(store.queries).toEqual([]);
    expect(read()).toBeNull();
  });

  it("records most recent first and round-trips through a fresh instance", () => {
    store.record("mercy", 100);
    store.record("light", 200);
    expect(store.queries).toEqual(["light", "mercy"]);
    expect(read()).toEqual({
      v: 1,
      entries: [
        { q: "light", at: 200 },
        { q: "mercy", at: 100 },
      ],
      paused: false,
    });
    const fresh = new SearchRecentsStore();
    expect(fresh.queries).toEqual(["light", "mercy"]);
    fresh.dispose();
  });

  it("repeating the top query refreshes its timestamp without duplicating", () => {
    store.record("mercy", 100);
    store.record("mercy", 300);
    expect(store.queries).toEqual(["mercy"]);
    expect(store.entries[0]?.at).toBe(300);
  });

  it("re-searching an older query moves it to the front", () => {
    store.record("a", 1);
    store.record("b", 2);
    store.record("c", 3);
    store.record("a", 4);
    expect(store.queries).toEqual(["a", "c", "b"]);
  });

  it("caps the list at the recents max, dropping the oldest", () => {
    for (let i = 0; i < SEARCH_RECENTS_MAX + 4; i += 1) store.record(`q${i}`, i);
    expect(store.queries.length).toBe(SEARCH_RECENTS_MAX);
    expect(store.queries[0]).toBe(`q${SEARCH_RECENTS_MAX + 3}`);
    expect(store.queries).not.toContain("q0");
  });

  it("trims and rejects blank or oversized queries", () => {
    store.record("  mercy  ", 1);
    expect(store.queries).toEqual(["mercy"]);
    store.record("   ", 2);
    store.record("x".repeat(129), 3);
    expect(store.queries).toEqual(["mercy"]);
  });

  it("clear empties the list and persists the empty shape", () => {
    store.record("mercy", 1);
    store.clear();
    expect(store.queries).toEqual([]);
    expect(read()).toEqual({ v: 1, entries: [], paused: false });
  });

  it("pause stops recording but keeps stored queries visible", () => {
    store.record("mercy", 1);
    store.setPaused(true);
    expect(store.paused).toBe(true);
    store.record("light", 2);
    expect(store.queries).toEqual(["mercy"]);
    store.setPaused(false);
    store.record("light", 3);
    expect(store.queries).toEqual(["light", "mercy"]);
  });

  it("drops malformed and duplicate entries during decode and keeps order", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        v: 1,
        entries: [
          { q: "keep", at: 5 },
          { q: "", at: 6 },
          { q: "keep", at: 7 },
          { junk: true },
          "nope",
        ],
        paused: true,
      }),
    );
    const fresh = new SearchRecentsStore();
    expect(fresh.queries).toEqual(["keep"]);
    expect(fresh.entries[0]?.at).toBe(5);
    expect(fresh.paused).toBe(true);
    fresh.dispose();
  });

  it("reconciles from a foreign-tab storage event", () => {
    store.record("mercy", 1);
    seed([{ q: "light", at: 9 }]);
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(store.queries).toEqual(["light"]);
    // Events for other keys change nothing.
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.prefs" }));
    expect(store.queries).toEqual(["light"]);
  });

  it("reads nothing on the server and never persists there (browser false)", () => {
    flag.value = false;
    const serverStore = new SearchRecentsStore();
    expect(serverStore.queries).toEqual([]);
    serverStore.record("mercy");
    serverStore.clear();
    serverStore.setPaused(true);
    expect(window.localStorage.getItem(KEY)).toBeNull();
    serverStore.dispose();
  });
});
