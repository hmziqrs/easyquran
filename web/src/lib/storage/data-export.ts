import {
  decodeBookmark,
  decodeBookmarkFolder,
  verseKeyOf,
  type Bookmark,
  type BookmarkFolder,
} from "#lib/bookmarks/schema.js";
import { bookmarks, type BookmarkImportCounts } from "#lib/bookmarks/store.svelte.js";
import type { VerseKey } from "#lib/data/quran.js";
import {
  decodeCards,
  homeCards,
  SCHEMA_VERSION as CARDS_SCHEMA_VERSION,
  type HomeCard,
} from "#lib/home/cards.svelte.js";
import { decodeThread, type ReadingThread } from "#lib/sessions/schema.js";
import { sessions } from "#lib/sessions/store.svelte.js";
import {
  decodeSettingsDoc,
  toSettingsDoc,
  applySettingsDoc,
  type SettingsDoc,
} from "#lib/settings/settings-document.js";
import { asArray, asNumber, asObject, asString } from "#lib/storage/decoders.js";
import { reader } from "#lib/stores/reader.svelte.js";

/**
 * Personal data export and import (F6). One versioned JSON file in Settings.
 *
 * Export leaves out device ids, sync stamps and `dirty`/`rejected` flags: those
 * belong to this install, not to the person's data. Import never overwrites a row
 * by id. It adds only ids this device lacks, through the stores, so the added rows
 * get stamps and `dirty` like any other write (contracts/sync.md). Settings are the
 * one exception: they are a document, applied whole, and `applySettingsDoc` keeps
 * consent off.
 */

export const EXPORT_FORMAT = "easyquran.export";
/** Highest export version this build reads. A newer file is refused, never guessed at. */
export const EXPORT_VERSION = 1;
/** Import guard against oversized note values. Not a product limit shown to people. */
const NOTE_TEXT_MAX = 20_000;

export interface ExportNote {
  readonly surah: number;
  readonly ayah: number;
  readonly text: string;
  readonly sourceId: string | null;
  readonly updatedAt: number | null;
}

/** A thread as exported: content and lifecycle only, no device id or stamps. */
export type ExportThread = Omit<
  ReadingThread,
  | "identityStamp"
  | "cursorStamp"
  | "lifecycleStamp"
  | "deviceId"
  | "deviceName"
  | "dirty"
  | "rejected"
  | "extras"
>;
export type ExportFolder = Omit<
  BookmarkFolder,
  "nameStamp" | "positionStamp" | "deletedStamp" | "deleted" | "dirty" | "rejected"
>;
export type ExportBookmark = Omit<Bookmark, "stamp" | "deleted" | "dirty" | "rejected">;

export interface ExportBundle {
  readonly format: typeof EXPORT_FORMAT;
  readonly version: number;
  readonly exportedAt: string;
  readonly settings: SettingsDoc;
  readonly folders: readonly ExportFolder[];
  readonly bookmarks: readonly ExportBookmark[];
  readonly threads: readonly ExportThread[];
  readonly notes: readonly ExportNote[];
  readonly cards: readonly HomeCard[];
}

/** What this device holds, read by the caller. Export takes it as data, so it stays testable. */
export interface ExportSources {
  readonly settings: SettingsDoc;
  readonly folders: readonly BookmarkFolder[];
  readonly bookmarks: readonly Bookmark[];
  readonly threads: readonly ReadingThread[];
  readonly notes: readonly ExportNote[];
  readonly cards: readonly HomeCard[];
}

export function buildExportBundle(sources: ExportSources, now: Date): ExportBundle {
  return {
    format: EXPORT_FORMAT,
    version: EXPORT_VERSION,
    exportedAt: now.toISOString(),
    settings: sources.settings,
    folders: sources.folders
      .filter((f) => !f.deleted)
      .map((f) => ({
        id: f.id,
        name: f.name,
        isDefault: f.isDefault,
        position: f.position,
        createdAt: f.createdAt,
        updatedAt: f.updatedAt,
      })),
    bookmarks: sources.bookmarks
      .filter((b) => !b.deleted)
      .map((b) => ({
        id: b.id,
        folderId: b.folderId,
        surah: b.surah,
        ayah: b.ayah,
        endSurah: b.endSurah,
        endAyah: b.endAyah,
        sourceId: b.sourceId,
        createdAt: b.createdAt,
        updatedAt: b.updatedAt,
      })),
    threads: sources.threads
      .filter((t) => t.state !== "deleted")
      .map((t) => ({
        id: t.id,
        origin: t.origin,
        originRef: t.originRef,
        scope: t.scope,
        boundEnd: t.boundEnd,
        surah: t.surah,
        ayah: t.ayah,
        localPage: t.localPage,
        ratio: t.ratio,
        sourceId: t.sourceId,
        device: t.device,
        startedAt: t.startedAt,
        updatedAt: t.updatedAt,
        endedAt: t.endedAt,
        endReason: t.endReason,
        state: t.state,
      })),
    notes: [...sources.notes],
    cards: [...sources.cards],
  };
}

export type ParseFailure = "not-export" | "malformed" | "future-version";

export interface ParsedImport {
  readonly settings: SettingsDoc;
  readonly folders: readonly BookmarkFolder[];
  readonly bookmarks: readonly Bookmark[];
  readonly threads: readonly ReadingThread[];
  readonly notes: readonly ExportNote[];
  readonly cards: readonly HomeCard[];
  /** Rows that failed their decoder and were dropped. The rest of the file still imports. */
  readonly invalid: number;
}

export type ParseOutcome =
  | { readonly ok: true; readonly value: ParsedImport }
  | { readonly ok: false; readonly reason: ParseFailure };

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed note from the import file; this decoder is its boundary parser.
function decodeNote(raw: unknown): ExportNote | undefined {
  const obj = asObject(raw);
  const surah = asNumber(obj?.surah, 1, 114);
  const ayah = asNumber(obj?.ayah, 1, 286);
  const text = asString(obj?.text);
  if (obj === undefined || surah === undefined || ayah === undefined) return undefined;
  if (text === undefined || text.length === 0 || text.length > NOTE_TEXT_MAX) return undefined;
  const sourceId = asString(obj.sourceId);
  return {
    surah,
    ayah,
    text,
    sourceId: sourceId !== undefined && sourceId.length > 0 ? sourceId : null,
    updatedAt: asNumber(obj.updatedAt, 0, Number.MAX_SAFE_INTEGER) ?? null,
  };
}

/** Length of a value that may not be an array; 0 for anything else. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed collection from the import file; this helper is its boundary check.
function lengthOf(raw: unknown): number {
  return Array.isArray(raw) ? raw.length : 0;
}

/** Structure first (format, version, arrays), then each row. Bad rows drop; a bad file is refused. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the parsed import file (untyped JSON); this function is the boundary parser.
export function parseExportBundle(raw: unknown): ParseOutcome {
  const obj = asObject(raw);
  if (obj === undefined) return { ok: false, reason: "malformed" };
  if (obj.format !== EXPORT_FORMAT) return { ok: false, reason: "not-export" };
  const version = asNumber(obj.version, 1, Number.MAX_SAFE_INTEGER);
  if (version === undefined || !Number.isInteger(version))
    return { ok: false, reason: "malformed" };
  if (version > EXPORT_VERSION) return { ok: false, reason: "future-version" };
  const settings = decodeSettingsDoc(obj.settings);
  const lists = [obj.folders, obj.bookmarks, obj.threads, obj.notes, obj.cards];
  if (settings === null || !lists.every((list) => Array.isArray(list))) {
    return { ok: false, reason: "malformed" };
  }
  const folders = asArray(obj.folders, (row) => decodeBookmarkFolder(row) ?? undefined);
  const items = asArray(obj.bookmarks, (row) => decodeBookmark(row) ?? undefined);
  const threads = asArray(obj.threads, (row) => {
    const thread = decodeThread(row);
    return thread === null || thread.state === "deleted" ? undefined : thread;
  });
  const notes = asArray(obj.notes, decodeNote);
  const cards = decodeCards({ v: CARDS_SCHEMA_VERSION, cards: obj.cards });
  const invalid =
    lengthOf(obj.folders) -
    folders.length +
    (lengthOf(obj.bookmarks) - items.length) +
    (lengthOf(obj.threads) - threads.length) +
    (lengthOf(obj.notes) - notes.length) +
    (lengthOf(obj.cards) - cards.length);
  return {
    ok: true,
    value: { settings, folders, bookmarks: items, threads, notes, cards, invalid },
  };
}

/** The stores an import writes through. Injected so the rules can be tested without the app singletons. */
export interface ImportPorts {
  readonly bookmarks: {
    importRows(
      folders: readonly BookmarkFolder[],
      items: readonly Bookmark[],
    ): BookmarkImportCounts;
  };
  readonly sessions: { importThreads(threads: readonly ReadingThread[]): number };
  readonly cards: {
    readonly list: readonly HomeCard[];
    add(card: HomeCard): boolean;
  };
  readonly notes: {
    getNote(key: VerseKey): string;
    setNote(key: VerseKey, text: string, sourceId: string | null): void;
  };
  readonly applySettings: (doc: SettingsDoc) => void;
}

export interface ImportSummary {
  readonly added: number;
  readonly skipped: number;
  readonly invalid: number;
}

/** Add what this device lacks. Returns totals for the toast; existing rows are never touched. */
export function applyImport(parsed: ParsedImport, ports: ImportPorts): ImportSummary {
  ports.applySettings(parsed.settings);
  const fromBookmarks = ports.bookmarks.importRows(parsed.folders, parsed.bookmarks);
  const threads = ports.sessions.importThreads(parsed.threads);
  let notes = 0;
  for (const note of parsed.notes) {
    const key = verseKeyOf(note.surah, note.ayah);
    if (ports.notes.getNote(key) !== "") continue;
    ports.notes.setNote(key, note.text, note.sourceId);
    notes += 1;
  }
  let cards = 0;
  const known = new Set(ports.cards.list.map((card) => card.id));
  for (const card of parsed.cards) {
    if (known.has(card.id)) continue;
    if (!ports.cards.add(card)) break;
    known.add(card.id);
    cards += 1;
  }
  const offered =
    parsed.folders.length +
    parsed.bookmarks.length +
    parsed.threads.length +
    parsed.notes.length +
    parsed.cards.length;
  const added = fromBookmarks.folders + fromBookmarks.bookmarks + threads + notes + cards;
  return { added, skipped: offered - added, invalid: parsed.invalid };
}

/** The live stores, wired for the Settings page. */
export function livePorts(): ImportPorts {
  return {
    bookmarks,
    sessions,
    cards: homeCards,
    notes: reader,
    applySettings: (doc) => applySettingsDoc(doc),
  };
}

/** Read everything this device holds for export. Call from the Settings page, never a derived. */
export function collectExportSources(): ExportSources {
  sessions.syncOwner();
  reader.syncNotes();
  return {
    settings: toSettingsDoc(),
    folders: [...bookmarks.folders],
    bookmarks: [...bookmarks.bookmarks],
    threads: [...sessions.all],
    notes: reader.noteEntries.map((entry) => ({
      surah: entry.surah,
      ayah: entry.ayah,
      text: entry.text,
      sourceId: entry.sourceId,
      updatedAt: entry.updatedAt,
    })),
    cards: [...homeCards.list],
  };
}

/** Save this device's export as one dated JSON file. Settings and the account delete flow both use it. */
export function downloadExportFile(now: Date = new Date()): void {
  const bundle = buildExportBundle(collectExportSources(), now);
  const blob = new Blob([JSON.stringify(bundle)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `easyquran-export-${now.toISOString().slice(0, 10)}.json`;
  document.body.append(link);
  link.click();
  link.remove();
  // Revoke after the browser has started the download; revoking inside the click can cancel it.
  setTimeout(() => URL.revokeObjectURL(url), 1_000);
}
