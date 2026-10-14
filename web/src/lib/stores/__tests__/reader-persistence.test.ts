import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/env", () => ({
  get browser() {
    return flag.value;
  },
}));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: "local",
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import type { AuthStatus } from "#lib/auth/auth-state.svelte.js";

import { createAnnotations } from "../annotations.svelte";
import { createReaderCore, type Persisted, READER_SCHEMA_VERSION } from "../reader-core.svelte";
import {
  createReaderPersistence,
  decodeNotes,
  decodeReader,
  type NotesOwnerSource,
} from "../reader-persistence.svelte";

const KEY = "easyquran.reader";
const NOTES_KEY_LOCAL = "easyquran.notes.local";

interface FakeAuthState {
  status: AuthStatus;
  user: { id: number } | null;
}

/** Mutable auth stand-in: persistence resolves the notes owner from it. */
function fakeAuth(initial: AuthStatus = "anonymous") {
  const source: FakeAuthState = {
    status: initial,
    user: null,
  };
  const stub: NotesOwnerSource = {
    get status() {
      return source.status;
    },
    get user() {
      return source.user;
    },
  };
  return { source, stub };
}

function readNotes(owner: string = "local"): { v: number; notes: Record<string, string> } | null {
  return JSON.parse(window.localStorage.getItem(`easyquran.notes.${owner}`) ?? "null");
}

describe("decodeReader", () => {
  it("preserves a fully valid blob and ignores the retired v≤5 position/bookmark fields", () => {
    const out = decodeReader({
      v: 1,
      current: 2,
      fontSize: 40,
      mode: "reading",
      bookmarks: { "1:1": true },
      notes: { "3:3": "hi" },
      lastRead: { num: 4, n: 5 },
    });
    expect(out).toEqual({ current: 2, fontSize: 40, mode: "reading", notes: { "3:3": "hi" } });
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

  it("returns {} for non-object blobs", () => {
    expect(decodeReader(null)).toEqual({});
    expect(decodeReader("nope")).toEqual({});
    expect(decodeReader(undefined)).toEqual({});
  });

  it("upgrades a v1 blob keeping user data; new fields absent until defaulted on hydrate", () => {
    const out = decodeReader({ v: 1, current: 2, notes: { "2:255": "kursi" } });
    expect(out).toEqual({ current: 2, notes: { "2:255": "kursi" } });
    expect(out.translationFamily).toBeUndefined();
  });

  it("decodes v3 arabicFont from the allowlist and drops unknown ids", () => {
    expect(decodeReader({ v: 3, arabicFont: "scheherazade-new" }).arabicFont).toBe(
      "scheherazade-new",
    );
    expect(decodeReader({ v: 3, arabicFont: "noto-naskh-arabic" }).arabicFont).toBe(
      "noto-naskh-arabic",
    );
    expect(decodeReader({ v: 3, arabicFont: "amiri" }).arabicFont).toBe("amiri");
    expect(decodeReader({ v: 3, arabicFont: "kufi" }).arabicFont).toBeUndefined();
    expect(decodeReader({ v: 3, arabicFont: 7 }).arabicFont).toBeUndefined();
  });

  it("decodes the v4 arabicScript field with allowlist + v3 default", () => {
    expect(decodeReader({ arabicScript: "tajweed" }).arabicScript).toBe("tajweed");
    expect(decodeReader({ arabicScript: "indopak" }).arabicScript).toBe("indopak");
    // v3 blobs predate the field: decode stays silent, defaults apply downstream.
    expect(decodeReader({ v: 3 }).arabicScript).toBeUndefined();
    expect(decodeReader({ arabicScript: "coptic" }).arabicScript).toBeUndefined();
  });

  it("decodes the v5 translationFamily with an allowlist", () => {
    expect(decodeReader({ translationFamily: "serif" }).translationFamily).toBe("serif");
    expect(decodeReader({ translationFamily: "mono" }).translationFamily).toBeUndefined();
    // v4 blobs predate the field: decode stays silent, defaults apply downstream.
    expect(decodeReader({ v: 4 }).translationFamily).toBeUndefined();
  });

  it("decodeNotes accepts the v1 envelope and rejects everything else", () => {
    expect(decodeNotes({ v: 1, notes: { "1:1": "hi" } })).toEqual({ v: 1, notes: { "1:1": "hi" } });
    expect(decodeNotes({ v: 2, notes: {} })).toBeUndefined();
    expect(decodeNotes({ notes: { "1:1": "hi" } })).toBeUndefined();
    expect(decodeNotes("nope")).toBeUndefined();
    expect(decodeNotes(null)).toBeUndefined();
    expect(decodeNotes({ v: 1, notes: 5 })?.notes).toEqual({});
  });

  it("decodes v3 translationSize only inside the 13-28 bounds", () => {
    expect(decodeReader({ v: 3, translationSize: 13 }).translationSize).toBe(13);
    expect(decodeReader({ v: 3, translationSize: 28 }).translationSize).toBe(28);
    expect(decodeReader({ v: 3, translationSize: 12 }).translationSize).toBeUndefined();
    expect(decodeReader({ v: 3, translationSize: 29 }).translationSize).toBeUndefined();
    expect(decodeReader({ v: 3, translationSize: "17" }).translationSize).toBeUndefined();
  });

  it("handles schema versions explicitly: past/current decode, future decodes known fields, garbage rejects", () => {
    expect(decodeReader({ v: 1, current: 2, arabicFont: "amiri" })).toMatchObject({
      current: 2,
      arabicFont: "amiri",
    });
    expect(decodeReader({ v: 3, current: 2, translationSize: 18 })).toMatchObject({
      current: 2,
      translationSize: 18,
    });
    expect(
      decodeReader({ v: READER_SCHEMA_VERSION + 1, current: 3, arabicFont: "noto-naskh-arabic" }),
    ).toMatchObject({ current: 3, arabicFont: "noto-naskh-arabic" });
    expect(decodeReader({ v: "2", current: 2 })).toEqual({});
    expect(decodeReader({ v: 0, current: 2 })).toEqual({});
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

  it("round-trips translationFamily through writeBlob and re-decode", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    core.s.arabicFont = "scheherazade-new";
    core.s.translationSize = 21;
    core.s.translationFamily = "serif";
    persistence.writeNow();
    // SAFETY: blob was just written by writeNow() from core.s, whose serialized shape is Persisted.
    const stored = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as Persisted;
    expect(stored.arabicFont).toBe("scheherazade-new");
    expect(stored.translationSize).toBe(21);
    expect(stored.translationFamily).toBe("serif");
    // v5: notes never live in the blob — only under easyquran.notes.{owner}.
    expect(stored).not.toHaveProperty("notes");
    const decoded = decodeReader(stored);
    expect(decoded.arabicFont).toBe("scheherazade-new");
    expect(decoded.translationSize).toBe(21);
    expect(decoded.translationFamily).toBe("serif");

    const next = createReaderCore();
    const nextPersistence = createReaderPersistence(next);
    nextPersistence.hydrate();
    expect(next.s.arabicFont).toBe("scheherazade-new");
    expect(next.s.translationSize).toBe(21);
    expect(next.s.translationFamily).toBe("serif");
    nextPersistence.dispose();
    persistence.dispose();
  });

  it("writeBlob keeps v4 defaults for untouched typography fields", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    persistence.writeNow();
    expect(read()).toMatchObject({
      v: 6,
      arabicFont: "kfgqpc-hafs",
      // Untouched blob carries the v4 default script (annotated Uthmani).
      arabicScript: "uthmani-annotated",
      translationSize: 17,
      translationFamily: "sans",
    });
    persistence.dispose();
  });

  it("scheduleNoteWrite() only writes the notes key after the trailing debounce", () => {
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    core.s.notes["1:1"] = "first";
    persistence.scheduleNoteWrite();
    vi.advanceTimersByTime(399);
    expect(window.localStorage.getItem(NOTES_KEY_LOCAL)).toBeNull();
    vi.advanceTimersByTime(1);
    expect(readNotes()).toEqual({ v: 1, notes: { "1:1": "first" } });
    persistence.dispose();
  });

  it("writeNow() flushes a pending note write instead of dropping it (own key, no blob write)", () => {
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    core.s.notes["2:2"] = "a";
    persistence.scheduleNoteWrite();
    core.s.current = 3;
    persistence.writeNow();
    expect(read()).toMatchObject({ current: 3 });
    expect(read()).not.toHaveProperty("notes");
    expect(readNotes()).toEqual({ v: 1, notes: { "2:2": "a" } });
    const blobAfter = window.localStorage.getItem(KEY);
    const notesAfter = window.localStorage.getItem(NOTES_KEY_LOCAL);
    vi.advanceTimersByTime(1000);
    expect(window.localStorage.getItem(KEY)).toBe(blobAfter);
    expect(window.localStorage.getItem(NOTES_KEY_LOCAL)).toBe(notesAfter);
    persistence.dispose();
  });

  it("flushNoteWrite() writes the notes key immediately", () => {
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    core.s.notes["4:4"] = "x";
    persistence.scheduleNoteWrite();
    persistence.flushNoteWrite();
    expect(readNotes()).toEqual({ v: 1, notes: { "4:4": "x" } });
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

  it("writeNow() before hydrate() defers and never wipes stored notes", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ v: 1, current: 2, notes: { "2:255": "kursi" } }),
    );
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    core.s.current = 5;
    persistence.writeNow();
    expect(read()).toMatchObject({ current: 2, notes: { "2:255": "kursi" } });
    persistence.hydrate();
    expect(core.s.notes).toEqual({ "2:255": "kursi" });
    expect(core.s.current).toBe(5);
    expect(read()).toMatchObject({ current: 5 });
    // The rewritten blob drops the notes field; the legacy data was seeded
    // into the per-owner notes key instead.
    expect(read()).not.toHaveProperty("notes");
    expect(readNotes()).toEqual({ v: 1, notes: { "2:255": "kursi" } });
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

describe("createReaderPersistence cross-tab re-apply", () => {
  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
  });

  it("re-applies v3 typography from a foreign-tab storage event", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    expect(core.s.arabicFont).toBe("kfgqpc-hafs");
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        v: 3,
        arabicFont: "scheherazade-new",
        translationSize: 21,
        mode: "reading",
      }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(core.s.arabicFont).toBe("scheherazade-new");
    expect(core.s.translationSize).toBe(21);
    expect(core.s.mode).toBe("reading");
    const root = document.documentElement;
    expect(root.dataset.arabicFont).toBe("scheherazade-new");
    expect(root.style.getPropertyValue("--reader-arabic-family")).toContain("Scheherazade New");
    expect(root.style.getPropertyValue("--reader-translation-size")).toBe("21px");
    persistence.dispose();
  });

  it("ignores storage events for other keys", () => {
    const core = createReaderCore();
    const persistence = createReaderPersistence(core);
    persistence.hydrate();
    window.localStorage.setItem("easyquran.prefs", JSON.stringify({ theme: "light" }));
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.prefs" }));
    expect(core.s.arabicFont).toBe("kfgqpc-hafs");
    expect(core.s.translationSize).toBe(17);
    persistence.dispose();
  });

  it("survives a previous-build tab rewriting the blob v4-shaped (EC-54)", () => {
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    core.s.translationFamily = "serif";
    persistence.writeNow();
    // A tab still running the v4 build rewrites the blob with the v4 shape:
    // no translationFamily, notes back inside the blob.
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ v: 4, bookmarks: { "2:255": true }, notes: { "2:255": "kursi" } }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    // Absent fields decode as undefined and applyPersisted skips them, so the
    // in-memory state survives the stale write.
    expect(core.s.translationFamily).toBe("serif");
    // Legacy blob notes are migration input only — never re-applied to memory (EC-17).
    expect(core.s.notes).toEqual({});
    persistence.dispose();
  });
});

describe("createReaderPersistence per-owner notes (v5, EC-17)", () => {
  beforeEach(() => {
    flag.value = true;
    vi.useFakeTimers();
    window.localStorage.clear();
  });
  afterEach(() => vi.useRealTimers());

  it("seeds the owner's key from legacy blob notes exactly once (marker blocks re-seed)", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ v: 4, bookmarks: { "2:255": true }, notes: { "2:255": "kursi" } }),
    );
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    expect(core.s.notes).toEqual({ "2:255": "kursi" });
    expect(readNotes()).toEqual({ v: 1, notes: { "2:255": "kursi" } });

    // The user clears their notes; the never-deleted legacy blob must not re-seed.
    window.localStorage.setItem(NOTES_KEY_LOCAL, JSON.stringify({ v: 1, notes: {} }));
    const second = createReaderCore();
    const secondPersistence = createReaderPersistence(second, stub);
    secondPersistence.hydrate();
    expect(second.s.notes).toEqual({});
    secondPersistence.dispose();
    persistence.dispose();
  });

  it("defers the first notes load while auth is unresolved", () => {
    const { source, stub } = fakeAuth("unknown");
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    expect(core.s.notes).toEqual({});
    expect(window.localStorage.getItem(NOTES_KEY_LOCAL)).toBeNull();
    // Resolving to anonymous loads the (empty) local owner state.
    source.status = "anonymous";
    persistence.syncNotesOwner();
    expect(core.s.notes).toEqual({});
    persistence.dispose();
  });

  it("parks each owner's notes on its own key; swaps never merge across owners (EC-17)", () => {
    const { source, stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    const annotations = createAnnotations(core, persistence);
    persistence.hydrate();

    annotations.setNote("1:1", "anon note");
    persistence.writeNow();
    expect(readNotes()).toEqual({ v: 1, notes: { "1:1": "anon note" } });

    // Sign in as user 7: local notes stay parked on the local key, the account
    // key loads (empty), and nothing from `local` leaks into it.
    source.status = "authenticated";
    source.user = { id: 7 };
    persistence.syncNotesOwner();
    expect(core.s.notes).toEqual({});
    annotations.setNote("2:2", "account note");
    persistence.writeNow();
    expect(readNotes("7")).toEqual({ v: 1, notes: { "2:2": "account note" } });
    expect(readNotes()).toEqual({ v: 1, notes: { "1:1": "anon note" } });

    // Sign out: account notes hidden, local notes return unchanged.
    source.status = "anonymous";
    source.user = null;
    persistence.syncNotesOwner();
    expect(core.s.notes).toEqual({ "1:1": "anon note" });
    persistence.dispose();
  });

  it("getNote() surfaces the current owner's notes after a transition (read path, EC-17)", () => {
    window.localStorage.setItem(
      "easyquran.notes.7",
      JSON.stringify({ v: 1, notes: { "3:3": "account" } }),
    );
    window.localStorage.setItem(
      NOTES_KEY_LOCAL,
      JSON.stringify({ v: 1, notes: { "3:3": "local" } }),
    );
    const { source, stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    const annotations = createAnnotations(core, persistence);
    persistence.hydrate();
    expect(annotations.getNote("3:3")).toBe("local");
    source.status = "authenticated";
    source.user = { id: 7 };
    expect(annotations.getNote("3:3")).toBe("account");
    persistence.dispose();
  });

  it("re-applies notes from a foreign-tab storage event on the active owner key", () => {
    const { stub } = fakeAuth();
    const core = createReaderCore();
    const persistence = createReaderPersistence(core, stub);
    persistence.hydrate();
    window.localStorage.setItem(
      NOTES_KEY_LOCAL,
      JSON.stringify({ v: 1, notes: { "9:9": "tab2" } }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: NOTES_KEY_LOCAL }));
    expect(core.s.notes).toEqual({ "9:9": "tab2" });
    persistence.dispose();
  });
});
