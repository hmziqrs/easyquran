import {
  asArray,
  asBooleanRecord,
  asLiteral,
  asNumber,
  asObject,
  asStringRecord,
  onPageHide,
  onStorageKey,
  readJSON,
  trailingDebounce,
  writeJSON,
} from "$lib/storage";
import {
  ARABIC_FONT_MAX,
  ARABIC_FONT_MIN,
  NOTE_PERSIST_DEBOUNCE_MS,
  SURAH_COUNT,
  READER_MODE_VALUES,
  type Persisted,
  type ReaderCore,
  type ReaderMode,
  type RecentsEntry,
  type SurahProgress,
} from "./reader-core.svelte";
import { applyReaderPresentation } from "./reader-presentation";

const STORAGE_KEY = "easyquran.reader";
const ANCHOR_PERSIST_DEBOUNCE_MS = 300;

export function decodeReader(raw: unknown): Partial<Persisted> {
  const stored = asObject(raw);
  if (!stored) return {};

  const out: Partial<Persisted> = {};

  const current = asNumber(stored.current, 1, SURAH_COUNT);
  if (current !== undefined) out.current = current;

  const fontSize = asNumber(stored.fontSize, ARABIC_FONT_MIN, ARABIC_FONT_MAX);
  if (fontSize !== undefined) out.fontSize = fontSize;

  const mode = asLiteral<ReaderMode>(stored.mode, READER_MODE_VALUES);
  if (mode) out.mode = mode;

  const bookmarksObj = asObject(stored.bookmarks);
  if (bookmarksObj) out.bookmarks = asBooleanRecord(bookmarksObj);

  const notesObj = asObject(stored.notes);
  if (notesObj) out.notes = asStringRecord(notesObj);

  if (stored.lastRead === null) {
    out.lastRead = null;
  } else {
    const lr = asObject(stored.lastRead);
    if (lr) {
      const num = asNumber(lr.num, -Infinity, Infinity);
      const n = asNumber(lr.n, -Infinity, Infinity);
      const sourceId = typeof lr.sourceId === "string" ? lr.sourceId : undefined;
      if (num !== undefined && n !== undefined)
        out.lastRead = sourceId !== undefined ? { num, n, sourceId } : { num, n };
    }
  }

  if (stored.lastReadAnchor === null) {
    out.lastReadAnchor = null;
  } else {
    const a = asObject(stored.lastReadAnchor);
    if (a) {
      const verseKey = typeof a.verseKey === "string" ? a.verseKey : undefined;
      const localPage = asNumber(a.localPage, 1, Number.POSITIVE_INFINITY);
      const ratio = asNumber(a.ratio, 0, 1);
      if (verseKey && localPage !== undefined && ratio !== undefined)
        out.lastReadAnchor = { verseKey, localPage, ratio };
    }
  }

  if (Array.isArray(stored.recents)) {
    out.recents = asArray<RecentsEntry>(stored.recents, (item) => {
      const e = asObject(item);
      if (!e) return undefined;
      const num = asNumber(e.num, 1, SURAH_COUNT);
      const n = asNumber(e.n, 1, Number.POSITIVE_INFINITY);
      const ts = asNumber(e.ts, 0, Number.POSITIVE_INFINITY);
      if (num === undefined || n === undefined || ts === undefined) return undefined;
      const sourceId = typeof e.sourceId === "string" ? e.sourceId : undefined;
      return sourceId !== undefined ? { num, n, sourceId, ts } : { num, n, ts };
    });
  }

  const progressObj = asObject(stored.progress);
  if (progressObj) {
    const progress: Record<number, SurahProgress> = {};
    for (const [k, v] of Object.entries(progressObj)) {
      const num = Number(k);
      if (!Number.isInteger(num) || num < 1) continue;
      const entry = asObject(v);
      if (!entry) continue;
      const furthestAyah = asNumber(entry.furthestAyah, 1, Number.POSITIVE_INFINITY);
      const ts = asNumber(entry.ts, 0, Number.POSITIVE_INFINITY);
      if (furthestAyah !== undefined && ts !== undefined)
        progress[num] = { furthestAyah, ts };
    }
    out.progress = progress;
  }

  return out;
}

function applyPersisted(s: ReaderCore["s"], p: Partial<Persisted>): void {
  if (p.current !== undefined) s.current = p.current;
  if (p.fontSize !== undefined) s.fontSize = p.fontSize;
  if (p.mode !== undefined) s.mode = p.mode;
  if (p.bookmarks !== undefined) s.bookmarks = p.bookmarks;
  if (p.notes !== undefined) s.notes = p.notes;
  if (p.lastRead !== undefined) s.lastRead = p.lastRead;
  if (p.lastReadAnchor !== undefined) s.lastReadAnchor = p.lastReadAnchor;
  if (p.recents !== undefined) s.recents = p.recents;
  if (p.progress !== undefined) s.progress = p.progress;
}

export interface ReaderPersistence {
  hydrate(): void;
  writeNow(): void;
  scheduleNoteWrite(): void;
  flushNoteWrite(): void;
  scheduleAnchorWrite(): void;
  flushAnchorWrite(): void;
  readonly hydrated: boolean;
  dispose(): void;
}

export function createReaderPersistence(core: ReaderCore): ReaderPersistence {
  const noteWriter = trailingDebounce(() => writeBlob(), NOTE_PERSIST_DEBOUNCE_MS);
  const anchorWriter = trailingDebounce(() => writeBlob(), ANCHOR_PERSIST_DEBOUNCE_MS);
  let hydrated = false;
  let dirty = false;
  let teardowns: Array<() => void> = [];

  function writeBlob(): void {
    if (!hydrated) {
      dirty = true;
      return;
    }
    const { v, current, fontSize, mode, bookmarks, notes, lastRead, lastReadAnchor, recents, progress } = core.s;
    writeJSON(STORAGE_KEY, { v, current, fontSize, mode, bookmarks, notes, lastRead, lastReadAnchor, recents, progress });
  }

  return {
    get hydrated() {
      return hydrated;
    },
    hydrate() {
      if (hydrated) return;
      hydrated = true;
      const stored = decodeReader(readJSON(STORAGE_KEY));
      if (dirty) stored.current = undefined;
      applyPersisted(core.s, stored);
      applyReaderPresentation(core.s.mode, core.s.fontSize);
      teardowns.push(
        onStorageKey(STORAGE_KEY, () => {
          applyPersisted(core.s, decodeReader(readJSON(STORAGE_KEY)));
          applyReaderPresentation(core.s.mode, core.s.fontSize);
        }),
      );
      teardowns.push(
        onPageHide(() => {
          noteWriter.flush();
          anchorWriter.flush();
        }),
      );
      if (dirty) {
        dirty = false;
        writeBlob();
      }
    },
    writeNow() {
      noteWriter.cancel();
      anchorWriter.cancel();
      writeBlob();
    },
    scheduleNoteWrite() {
      noteWriter.schedule();
    },
    flushNoteWrite() {
      noteWriter.flush();
    },
    scheduleAnchorWrite() {
      anchorWriter.schedule();
    },
    flushAnchorWrite() {
      anchorWriter.flush();
    },
    dispose() {
      noteWriter.flush();
      anchorWriter.flush();
      for (const teardown of teardowns) teardown();
      teardowns = [];
    },
  };
}
