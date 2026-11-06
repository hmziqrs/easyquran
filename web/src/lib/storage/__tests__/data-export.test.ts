import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { verseKeyOf, type Bookmark, type BookmarkFolder } from "#lib/bookmarks/schema.js";
import {
  BookmarksStore,
  type SyncEngineLike as BookmarksEngineLike,
} from "#lib/bookmarks/store.svelte.js";
import { DEFAULTS } from "#lib/config/site.js";
import type { SurahCard } from "#lib/home/cards.svelte.js";
import type { ReadingThread } from "#lib/sessions/schema.js";
import { SessionsStore, type SessionsEngineLike } from "#lib/sessions/store.svelte.js";
import {
  applySettingsDoc,
  decodeSettingsDoc,
  type SettingsDoc,
  type SettingsDocStores,
  type SettingsReaderPort,
} from "#lib/settings/settings-document.js";
import { asObject } from "#lib/storage/decoders.js";
import type { Prefs } from "#lib/stores/prefs.svelte.js";
import type { SyncStatus } from "#lib/sync/index.js";

import {
  EXPORT_FORMAT,
  EXPORT_VERSION,
  applyImport,
  buildExportBundle,
  parseExportBundle,
  type ExportNote,
  type ImportPorts,
  type ParsedImport,
} from "../data-export";

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));
vi.mock("#lib/stores/reader-presentation.js", () => ({ applyReaderPresentation: vi.fn() }));

const ID = {
  folder: "11111111-1111-4111-8111-111111111111",
  deletedFolder: "22222222-2222-4222-8222-222222222222",
  bookmark: "33333333-3333-4333-8333-333333333333",
  twin: "44444444-4444-4444-8444-444444444444",
  thread: "55555555-5555-4555-8555-555555555555",
  deletedThread: "66666666-6666-4666-8666-666666666666",
  card: "77777777-7777-4777-8777-777777777777",
};

const NOW = "2026-11-06T10:00:00.000Z";
const STATUS: SyncStatus = { phase: "idle", pending: 0, lastSyncAt: null, lastError: null };

function defaultSettings(): SettingsDoc {
  const doc = decodeSettingsDoc({});
  if (doc === null) throw new Error("default settings must decode");
  return doc;
}

function folder(over: Partial<BookmarkFolder> = {}): BookmarkFolder {
  return {
    id: ID.folder,
    name: "Hifz",
    isDefault: false,
    position: 2,
    createdAt: NOW,
    updatedAt: NOW,
    nameStamp: "1700000000000-0001-dev-a",
    positionStamp: "1700000000000-0002-dev-a",
    deletedStamp: null,
    deleted: false,
    dirty: true,
    rejected: false,
    ...over,
  };
}

function bookmark(over: Partial<Bookmark> = {}): Bookmark {
  return {
    id: ID.bookmark,
    folderId: ID.folder,
    surah: 2,
    ayah: 255,
    endSurah: null,
    endAyah: null,
    sourceId: null,
    createdAt: NOW,
    updatedAt: NOW,
    stamp: "1700000000000-0003-dev-a",
    deleted: false,
    dirty: true,
    rejected: false,
    ...over,
  };
}

function thread(over: Partial<ReadingThread> = {}): ReadingThread {
  return {
    deviceName: null,
    id: ID.thread,
    origin: "index",
    originRef: null,
    scope: "surah:2",
    boundEnd: null,
    surah: 2,
    ayah: 5,
    localPage: 3,
    ratio: 0.25,
    sourceId: null,
    device: "desktop",
    startedAt: NOW,
    updatedAt: NOW,
    endedAt: null,
    endReason: null,
    state: "open",
    deviceId: "dev-a",
    identityStamp: "1700000000000-0004-dev-a",
    cursorStamp: "1700000000000-0005-dev-a",
    lifecycleStamp: "1700000000000-0006-dev-a",
    dirty: true,
    rejected: false,
    extras: {},
    ...over,
  };
}

function surahCard(id: string): SurahCard {
  return { id, kind: "surah", count: 0, createdAt: NOW, surah: 36, sourceId: null };
}

function note(over: Partial<ExportNote> = {}): ExportNote {
  return { surah: 1, ayah: 1, text: "Opening", sourceId: null, updatedAt: null, ...over };
}

function sources(over: Partial<Parameters<typeof buildExportBundle>[0]> = {}) {
  return {
    settings: defaultSettings(),
    folders: [folder()],
    bookmarks: [bookmark()],
    threads: [thread()],
    notes: [note()],
    cards: [],
    ...over,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is parsed JSON from an export file; this helper is its boundary check.
function jsonRecord(raw: unknown) {
  return asObject(raw) ?? {};
}

/** A bundle as a file would carry it: built, serialized, parsed back. Callers spread patches over it. */
function exportRecord(over: Partial<Parameters<typeof buildExportBundle>[0]> = {}) {
  const text = JSON.stringify(buildExportBundle(sources(over), new Date(NOW)));
  return jsonRecord(JSON.parse(text));
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is a file's parsed JSON, possibly malformed; parseExportBundle is the boundary.
function parsed(raw: unknown): ParsedImport {
  const outcome = parseExportBundle(raw);
  if (!outcome.ok) throw new Error(`expected a readable export, got ${outcome.reason}`);
  return outcome.value;
}

describe("buildExportBundle", () => {
  it("writes format, version and time, and no device ids, stamps or queue flags", () => {
    const text = JSON.stringify(buildExportBundle(sources(), new Date(NOW)));
    expect(text).toContain(`"format":"${EXPORT_FORMAT}"`);
    expect(text).toContain(`"version":${EXPORT_VERSION}`);
    expect(text).toContain(`"exportedAt":"${NOW}"`);
    for (const forbidden of ["deviceId", "Stamp", "dirty", "rejected", "dev-a", "extras"]) {
      expect(text).not.toContain(forbidden);
    }
  });

  it("leaves out tombstones and deleted threads", () => {
    const bundle = buildExportBundle(
      sources({
        folders: [folder(), folder({ id: ID.deletedFolder, deleted: true })],
        bookmarks: [bookmark(), bookmark({ id: ID.twin, deleted: true })],
        threads: [thread(), thread({ id: ID.deletedThread, state: "deleted" })],
      }),
      new Date(NOW),
    );
    expect(bundle.folders.map((f) => f.id)).toEqual([ID.folder]);
    expect(bundle.bookmarks.map((b) => b.id)).toEqual([ID.bookmark]);
    expect(bundle.threads.map((t) => t.id)).toEqual([ID.thread]);
  });
});

describe("parseExportBundle", () => {
  it("reads back what it wrote", () => {
    const value = parsed(exportRecord());
    expect(value.folders.map((f) => f.id)).toEqual([ID.folder]);
    expect(value.bookmarks.map((b) => b.id)).toEqual([ID.bookmark]);
    expect(value.threads.map((t) => t.id)).toEqual([ID.thread]);
    expect(value.notes).toEqual([note()]);
    expect(value.invalid).toBe(0);
  });

  it("refuses non-objects, other formats and bad versions", () => {
    expect(parseExportBundle("text")).toEqual({ ok: false, reason: "malformed" });
    expect(parseExportBundle({ format: "something-else", version: 1 })).toEqual({
      ok: false,
      reason: "not-export",
    });
    expect(parseExportBundle({ ...exportRecord(), version: 1.5 })).toEqual({
      ok: false,
      reason: "malformed",
    });
  });

  it("refuses a newer schema version rather than guessing at it", () => {
    expect(parseExportBundle({ ...exportRecord(), version: EXPORT_VERSION + 1 })).toEqual({
      ok: false,
      reason: "future-version",
    });
  });

  it("refuses a file whose collections are not arrays", () => {
    expect(parseExportBundle({ ...exportRecord(), bookmarks: "nope" })).toEqual({
      ok: false,
      reason: "malformed",
    });
  });

  it("drops bad rows, keeps the rest, and counts them", () => {
    const value = parsed({
      ...exportRecord(),
      bookmarks: [bookmark(), { id: "short" }, { ...bookmark({ id: ID.twin }), surah: 999 }],
      notes: [note(), { surah: 1, ayah: 2, text: "" }],
    });
    expect(value.bookmarks.map((b) => b.id)).toEqual([ID.bookmark]);
    expect(value.notes).toHaveLength(1);
    expect(value.invalid).toBe(3);
  });
});

interface FakeState {
  folders: Set<string>;
  bookmarks: Set<string>;
  threads: Set<string>;
  notes: Set<string>;
  cards: SurahCard[];
}

/** Ports that record what an import wrote, with the same absent-id rules as the live stores. */
function fakePorts(existing: Partial<FakeState> = {}) {
  const state: FakeState = {
    folders: existing.folders ?? new Set(),
    bookmarks: existing.bookmarks ?? new Set(),
    threads: existing.threads ?? new Set(),
    notes: existing.notes ?? new Set(),
    cards: existing.cards ?? [],
  };
  const written = {
    folders: new Array<string>(),
    bookmarks: new Array<string>(),
    threads: new Array<string>(),
    notes: new Array<string>(),
    settings: 0,
  };
  const ports: ImportPorts = {
    bookmarks: {
      importRows(folders, items) {
        let folderCount = 0;
        for (const row of folders) {
          if (state.folders.has(row.id)) continue;
          state.folders.add(row.id);
          written.folders.push(row.id);
          folderCount += 1;
        }
        let rowCount = 0;
        for (const row of items) {
          if (state.bookmarks.has(row.id)) continue;
          state.bookmarks.add(row.id);
          written.bookmarks.push(row.id);
          rowCount += 1;
        }
        return { folders: folderCount, bookmarks: rowCount };
      },
    },
    sessions: {
      importThreads(list) {
        let added = 0;
        for (const row of list) {
          if (state.threads.has(row.id)) continue;
          state.threads.add(row.id);
          written.threads.push(row.id);
          added += 1;
        }
        return added;
      },
    },
    cards: {
      get list() {
        return state.cards;
      },
      add(next) {
        if (state.cards.length >= 2) return false;
        state.cards.push(surahCard(next.id));
        return true;
      },
    },
    notes: {
      getNote: (key) => (state.notes.has(key) ? "kept" : ""),
      setNote(key, text) {
        state.notes.add(key);
        written.notes.push(`${key}=${text}`);
      },
    },
    applySettings: () => {
      written.settings += 1;
    },
  };
  return { ports, written };
}

describe("applyImport", () => {
  it("adds only ids the device lacks and reports the totals", () => {
    const value = parsed(exportRecord());
    const { ports, written } = fakePorts({ bookmarks: new Set([ID.bookmark]) });
    const summary = applyImport(value, ports);
    expect(written.folders).toEqual([ID.folder]);
    expect(written.bookmarks).toEqual([]);
    expect(written.threads).toEqual([ID.thread]);
    expect(written.settings).toBe(1);
    // Folder, thread and note are new here; the bookmark already exists and is skipped.
    expect(summary).toEqual({ added: 3, skipped: 1, invalid: 0 });
  });

  it("never overwrites an existing note", () => {
    const value = parsed(exportRecord({ notes: [note({ text: "imported" })] }));
    const { ports, written } = fakePorts({ notes: new Set([verseKeyOf(1, 1)]) });
    applyImport(value, ports);
    expect(written.notes).toEqual([]);
  });

  it("stops adding cards at the cap and counts the rest as skipped", () => {
    const incoming = [surahCard("c1"), surahCard("c2"), surahCard("c3")];
    const value = parsed(exportRecord({ cards: incoming }));
    const { ports } = fakePorts();
    const summary = applyImport(value, ports);
    expect(ports.cards.list.map((c) => c.id)).toEqual(["c1", "c2"]);
    // Only the third card is skipped; everything else in the file is new and was added.
    expect(summary.skipped).toBe(1);
  });
});

describe("settings import", () => {
  it("never turns consent on, whatever the file says", () => {
    const base = defaultSettings();
    const doc: SettingsDoc = { ...base, privacy: { analytics: true, advertising: true } };
    const consentSets: unknown[] = [];
    const prefs: Prefs = { ...DEFAULTS, instantResume: false, custom: {} };
    const reader: SettingsReaderPort = {
      mode: base.reading.mode,
      arabicFont: base.reading.arabicFont,
      arabicScript: base.reading.arabicScript,
      translationFamily: base.reading.translationFamily,
      arabicSizePx: "32px",
      translationSizePx: "18px",
      setMode: () => undefined,
      setArabicFont: () => undefined,
      setArabicScript: () => undefined,
      setTranslationFamily: () => undefined,
      bigger: () => undefined,
      smaller: () => undefined,
      growTranslation: () => undefined,
      shrinkTranslation: () => undefined,
    };
    const stores: SettingsDocStores = {
      prefs: { current: prefs, set: () => undefined, apply: () => undefined },
      consent: {
        current: { analytics: false, advertising: false },
        set: (patch) => {
          consentSets.push(patch);
        },
      },
      reader,
    };
    applySettingsDoc(doc, stores);
    expect(consentSets.at(-1)).toEqual({ analytics: false, advertising: false });
  });
});

describe("local stores", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  function bookmarkStore(): BookmarksStore {
    const engine: BookmarksEngineLike = {
      flush: async () => undefined,
      start: () => () => undefined,
      status: STATUS,
    };
    const store = new BookmarksStore({
      engine,
      auth: { unsafeRequest: () => Promise.reject(new Error("anonymous: no requests")) },
      state: { authenticated: false, user: null },
      register: () => undefined,
      unregister: () => undefined,
    });
    store.hydrate();
    return store;
  }

  function sessionStore(): SessionsStore {
    const engine: SessionsEngineLike = {
      flush: async () => undefined,
      start: () => () => undefined,
    };
    return new SessionsStore({
      engine,
      auth: {
        unsafeRequest: () => Promise.reject(new Error("anonymous: no requests")),
        get: () => Promise.reject(new Error("anonymous: no requests")),
      },
      state: { authenticated: false, user: null },
      register: () => undefined,
      unregister: () => undefined,
    });
  }

  it("bookmarks: imports absent folders and rows, skips existing and live twins, files orphans in the default folder", () => {
    const store = bookmarkStore();
    const defaultId = store.defaultFolderId;
    const incomingFolders = [folder()];
    const incomingItems = [
      bookmark(),
      bookmark({ id: ID.twin }),
      bookmark({ id: ID.card, folderId: ID.deletedFolder }),
    ];
    expect(store.importRows(incomingFolders, incomingItems)).toEqual({ folders: 1, bookmarks: 2 });
    expect(store.folderById(ID.folder)?.name).toBe("Hifz");
    expect(store.bookmarks.map((b) => b.folderId).sort()).toEqual([ID.folder, defaultId].sort());
    expect(store.importRows(incomingFolders, incomingItems)).toEqual({ folders: 0, bookmarks: 0 });
  });

  it("sessions: imports absent threads, skips existing ids, uses this device's id", () => {
    const store = sessionStore();
    expect(
      store.importThreads([thread(), thread({ id: ID.deletedThread, state: "deleted" })]),
    ).toBe(1);
    expect(store.importThreads([thread()])).toBe(0);
    expect(store.all.map((t) => t.id)).toEqual([ID.thread]);
    expect(store.all[0]?.deviceId).toBeNull();
  });

  it("round trip: export a populated device, import into a clean one, nothing lost", () => {
    const origin = bookmarkStore();
    const created = origin.createFolder("Hifz");
    if (created === null) throw new Error("folder must be created");
    origin.setInFolder(2, 255, created.id, true);
    const originSessions = sessionStore();
    originSessions.importThreads([thread()]);

    const bundle = buildExportBundle(
      {
        settings: defaultSettings(),
        folders: [...origin.folders],
        bookmarks: [...origin.bookmarks],
        threads: [...originSessions.all],
        notes: [note()],
        cards: [],
      },
      new Date(NOW),
    );
    const transported = jsonRecord(JSON.parse(JSON.stringify(bundle)));

    window.localStorage.clear();
    const target = bookmarkStore();
    const targetSessions = sessionStore();
    const notes = new Map<string, string>();
    applyImport(parsed(transported), {
      bookmarks: target,
      sessions: targetSessions,
      cards: { list: [], add: () => true },
      notes: {
        getNote: (key) => notes.get(key) ?? "",
        setNote: (key, text) => {
          notes.set(key, text);
        },
      },
      applySettings: () => undefined,
    });

    expect(target.folderById(created.id)?.name).toBe("Hifz");
    expect(target.bookmarks.map((b) => [b.surah, b.ayah])).toEqual([[2, 255]]);
    expect(targetSessions.all.map((t) => t.id)).toEqual([ID.thread]);
    expect(notes.get(verseKeyOf(1, 1))).toBe("Opening");
  });
});
