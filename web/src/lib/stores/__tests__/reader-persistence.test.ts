import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

// Persistence flush integration runs against a real localStorage, so force the
// browser flag on. decodeReader tests are pure and don't touch storage.
const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/environment", () => ({
  get browser() {
    return flag.value;
  },
}));

import { createReaderCore, READER_SCHEMA_VERSION } from "../reader-core.svelte";
import { createReaderPersistence, decodeReader } from "../reader-persistence.svelte";

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

  it("discards a blob whose explicit version is in the future", () => {
    expect(decodeReader({ v: 99, current: 2 })).toEqual({});
    // a versioned-but-non-matching type also counts as future
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
    // Empty object is still a valid (empty) record.
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
});

describe("createReaderPersistence scheduling", () => {
  beforeEach(() => {
    flag.value = true;
    vi.useFakeTimers();
    window.localStorage.clear();
  });
  afterEach(() => vi.useRealTimers());

  const read = (): unknown => JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("writeNow() persists the durable blob immediately", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    core.s.current = 7;
    persistence.writeNow();
    expect(read()).toMatchObject({ v: READER_SCHEMA_VERSION, current: 7 });
  });

  it("scheduleNoteWrite() only writes after the trailing debounce", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate(); // also arms listeners; harmless here
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
    core.s.notes["2:2"] = "a";
    persistence.scheduleNoteWrite();
    // Discrete change writes immediately and should cancel the pending note write.
    core.s.bookmarks["3:3"] = true;
    persistence.writeNow();
    expect(read()).toMatchObject({ bookmarks: { "3:3": true }, notes: { "2:2": "a" } });
    // Advancing past the debounce window must not re-invoke the writer.
    const stampAfter = window.localStorage.getItem(KEY);
    vi.advanceTimersByTime(1000);
    expect(window.localStorage.getItem(KEY)).toBe(stampAfter);
    persistence.dispose();
  });

  it("flushNoteWrite() writes immediately", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
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
