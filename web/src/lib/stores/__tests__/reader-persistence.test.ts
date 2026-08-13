import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/environment", () => ({
  get browser() {
    return flag.value;
  },
}));

import { createReaderCore, type Persisted, READER_SCHEMA_VERSION } from "../reader-core.svelte";
import { createReaderPersistence, decodeReader } from "../reader-persistence.svelte";
import { createReader } from "../reader.svelte";

const KEY = "easyquran.reader";

describe("decodeReader", () => {
  it("preserves a fully valid blob", () => {
    const out = decodeReader({
      v: 1,
      current: 2,
      fontSize: 40,
      mode: "reading",
      bookmarks: { "1:1": true, "2:2": false },
      notes: { "3:3": "hi" },
      lastRead: { num: 4, n: 5 },
    });
    expect(out).toEqual({
      current: 2,
      fontSize: 40,
      mode: "reading",
      bookmarks: { "1:1": true },
      notes: { "3:3": "hi" },
      lastRead: { num: 4, n: 5 },
    });
  });

  it("decodes a future-version blob tolerantly (known fields kept, never wiped)", () => {
    expect(decodeReader({ v: 99, current: 2 })).toEqual({ current: 2 });
    expect(decodeReader({ v: "2" })).toEqual({});
  });

  it("migrates a legacy (versionless) blob forward", () => {
    const out = decodeReader({ current: 3, mode: "verse" });
    expect(out).toEqual({ current: 3, mode: "verse" });
  });

  it("drops out-of-range current/fontSize and invalid mode", () => {
    const out = decodeReader({
      current: 999,
      fontSize: 9,
      mode: "scrolled",
    });
    expect(out.current).toBeUndefined();
    expect(out.fontSize).toBeUndefined();
    expect(out.mode).toBeUndefined();
  });

  it("keeps bookmarks/notes records only when the stored value is an object", () => {
    expect(decodeReader({ bookmarks: "nope" }).bookmarks).toBeUndefined();
    expect(decodeReader({ notes: 5 }).notes).toBeUndefined();
    expect(decodeReader({ bookmarks: {} }).bookmarks).toEqual({});
  });

  it("filters non-true bookmarks and non-string notes", () => {
    const out = decodeReader({
      bookmarks: { "1:1": true, "2:2": 1, "3:3": "true" },
      notes: { "4:4": "ok", "5:5": 7 },
    });
    expect(out.bookmarks).toEqual({ "1:1": true });
    expect(out.notes).toEqual({ "4:4": "ok" });
  });

  it("decodes lastRead null / valid object / rejects garbage", () => {
    expect(decodeReader({ lastRead: null }).lastRead).toBeNull();
    expect(decodeReader({ lastRead: { num: 2, n: 3 } }).lastRead).toEqual({ num: 2, n: 3 });
    expect(decodeReader({ lastRead: { num: "x", n: 3 } }).lastRead).toBeUndefined();
    expect(decodeReader({ lastRead: [1, 2] }).lastRead).toBeUndefined();
  });

  it("returns {} for non-object blobs", () => {
    expect(decodeReader(null)).toEqual({});
    expect(decodeReader("nope")).toEqual({});
    expect(decodeReader(undefined)).toEqual({});
  });

  it("upgrades a v1 blob keeping user data; new fields absent until defaulted on hydrate", () => {
    const out = decodeReader({
      v: 1,
      current: 2,
      bookmarks: { "2:255": true },
      lastRead: { num: 2, n: 255, sourceId: "uthmani" },
    });
    expect(out).toEqual({
      current: 2,
      bookmarks: { "2:255": true },
      lastRead: { num: 2, n: 255, sourceId: "uthmani" },
    });
    expect(out.lastReadAnchor).toBeUndefined();
    expect(out.recents).toBeUndefined();
    expect(out.progress).toBeUndefined();
  });

  it("decodes lastReadAnchor null / valid / rejects garbage", () => {
    expect(decodeReader({ lastReadAnchor: null }).lastReadAnchor).toBeNull();
    expect(
      decodeReader({ lastReadAnchor: { verseKey: "1:1", localPage: 1, ratio: 0 } }).lastReadAnchor,
    ).toEqual({ verseKey: "1:1", localPage: 1, ratio: 0 });
    expect(
      decodeReader({ lastReadAnchor: { verseKey: 1, localPage: 1, ratio: 0 } }).lastReadAnchor,
    ).toBeUndefined();
    expect(
      decodeReader({ lastReadAnchor: { verseKey: "1:1", localPage: 1, ratio: 2 } }).lastReadAnchor,
    ).toBeUndefined();
  });

  it("drops malformed recents entries and progress keys, not the whole blob", () => {
    const out = decodeReader({
      recents: [{ num: 2, n: 255, ts: 1 }, { num: "x", n: 1, ts: 1 }, "nope"],
      progress: { "2": { furthestAyah: 5, ts: 1 }, bad: { furthestAyah: 1, ts: 1 } },
    });
    expect(out.recents).toEqual([{ num: 2, n: 255, ts: 1 }]);
    expect(out.progress).toEqual({ 2: { furthestAyah: 5, ts: 1 } });
  });
});

describe("createReaderPersistence scheduling", () => {
  beforeEach(() => {
    flag.value = true;
    vi.useFakeTimers();
    window.localStorage.clear();
  });
  afterEach(() => vi.useRealTimers());

  const read = (): Persisted | null => JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("writeNow() persists the durable blob immediately", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.current = 7;
    persistence.writeNow();
    expect(read()).toMatchObject({ v: READER_SCHEMA_VERSION, current: 7 });
    persistence.dispose();
  });

  it("scheduleAnchorWrite() only writes after the trailing debounce", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.lastReadAnchor = { verseKey: "2:255", localPage: 3, ratio: 0.5 };
    persistence.scheduleAnchorWrite();
    vi.advanceTimersByTime(299);
    expect(read()).toBeNull();
    vi.advanceTimersByTime(1);
    expect(read()).toMatchObject({
      lastReadAnchor: { verseKey: "2:255", localPage: 3, ratio: 0.5 },
    });
    persistence.dispose();
  });

  it("round-trips lastReadAnchor, recents, and progress through localStorage", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.lastReadAnchor = { verseKey: "2:255", localPage: 3, ratio: 0.5 };
    core.s.recents = [{ num: 2, n: 255, sourceId: "uthmani", ts: 1000 }];
    core.s.progress = { 2: { furthestAyah: 255, ts: 1000 } };
    persistence.writeNow();
    // SAFETY: blob was just written by writeNow() from core.s, whose serialized shape is Persisted.
    const stored = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as Persisted;
    expect(stored.v).toBe(READER_SCHEMA_VERSION);
    expect(stored.lastReadAnchor).toEqual({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    expect(stored.recents).toEqual([{ num: 2, n: 255, sourceId: "uthmani", ts: 1000 }]);
    expect(stored.progress).toEqual({ "2": { furthestAyah: 255, ts: 1000 } });
    const decoded = decodeReader(stored);
    expect(decoded.lastReadAnchor).toEqual({ verseKey: "2:255", localPage: 3, ratio: 0.5 });
    expect(decoded.recents).toEqual([{ num: 2, n: 255, sourceId: "uthmani", ts: 1000 }]);
    expect(decoded.progress).toEqual({ 2: { furthestAyah: 255, ts: 1000 } });
    persistence.dispose();
  });

  it("markRead records a per-surah high-water progress", () => {
    const r = createReader();
    r.hydrate();
    r.markRead(1, 5);
    r.markRead(1, 2);
    const blob = JSON.parse(window.localStorage.getItem(KEY) ?? "{}");
    expect(blob.progress["1"].furthestAyah).toBe(5);
  });

  it("scheduleNoteWrite() only writes after the trailing debounce", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.notes["1:1"] = "first";
    persistence.scheduleNoteWrite();
    vi.advanceTimersByTime(399);
    expect(read()).toBeNull();
    vi.advanceTimersByTime(1);
    expect(read()).toMatchObject({ notes: { "1:1": "first" } });
    persistence.dispose();
  });

  it("writeNow() cancels a pending debounced write (no redundant later write)", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.notes["2:2"] = "a";
    persistence.scheduleNoteWrite();
    core.s.bookmarks["3:3"] = true;
    persistence.writeNow();
    expect(read()).toMatchObject({ bookmarks: { "3:3": true }, notes: { "2:2": "a" } });
    const stampAfter = window.localStorage.getItem(KEY);
    vi.advanceTimersByTime(1000);
    expect(window.localStorage.getItem(KEY)).toBe(stampAfter);
    persistence.dispose();
  });

  it("flushNoteWrite() writes immediately", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.notes["4:4"] = "x";
    persistence.scheduleNoteWrite();
    persistence.flushNoteWrite();
    expect(read()).toMatchObject({ notes: { "4:4": "x" } });
    persistence.dispose();
  });

  it("hydrate() re-applies validated fields from localStorage", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ v: 1, current: 5, mode: "reading", fontSize: 44 }),
    );
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    expect(core.s.current).toBe(5);
    expect(core.s.mode).toBe("reading");
    expect(core.s.fontSize).toBe(44);
    persistence.dispose();
  });
});

describe("createReaderPersistence hydration race", () => {
  beforeEach(() => {
    flag.value = true;
    vi.useFakeTimers();
    window.localStorage.clear();
  });
  afterEach(() => vi.useRealTimers());

  const read = (): Persisted | null => JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("writeNow() before hydrate() defers and never wipes stored bookmarks/notes/lastRead", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        v: 1,
        current: 2,
        bookmarks: { "2:255": true },
        notes: { "2:255": "kursi" },
        lastRead: { num: 2, n: 255 },
      }),
    );
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    core.s.current = 5;
    persistence.writeNow();
    expect(read()).toMatchObject({
      current: 2,
      bookmarks: { "2:255": true },
      notes: { "2:255": "kursi" },
      lastRead: { num: 2, n: 255 },
    });
    persistence.hydrate();
    expect(core.s.bookmarks).toEqual({ "2:255": true });
    expect(core.s.notes).toEqual({ "2:255": "kursi" });
    expect(core.s.lastRead).toEqual({ num: 2, n: 255 });
    expect(core.s.current).toBe(5);
    expect(read()).toMatchObject({
      current: 5,
      bookmarks: { "2:255": true },
      notes: { "2:255": "kursi" },
    });
    persistence.dispose();
  });

  it("writeNow() before hydrate() is a no-op when nothing is stored, then reconciles on hydrate", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    core.s.current = 9;
    persistence.writeNow();
    expect(read()).toBeNull();
    persistence.hydrate();
    expect(core.s.current).toBe(9);
    expect(read()).toMatchObject({ current: 9 });
    persistence.dispose();
  });
});
