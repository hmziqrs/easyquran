/* ════════════════════════════════════════════════════════════════════════
   reader-persistence.svelte.ts — durable-state persistence for the reader.

   Owns the single localStorage key (`easyquran.reader`, schema-versioned v1)
   and ALL write scheduling:
     • writeNow()         — immediate full-blob write (bookmarks/font/mode/nav).
     • scheduleNoteWrite()— debounced trailing write (notes change per keystroke).
     • flushNoteWrite()   — flush pending debounced write (note close / page-hide).

   Cross-tab: subscribes to the `storage` event on hydrate so another tab's write
   re-syncs this tab. Page-hide: flushes a pending note write so the last
   keystroke is durable. Both listeners are torn down in dispose().

   The durable blob is a SINGLE record shared across the settings/annotations/
   navigation facets, so this module is the single writer — facets call its
   scheduling methods. Decode lives here too (validated via $lib/storage).
   ════════════════════════════════════════════════════════════════════════ */

import {
  asBooleanRecord,
  asLiteral,
  asNullableObject,
  asNumber,
  asObject,
  asStringRecord,
  isFutureSchema,
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
  READER_SCHEMA_VERSION,
  SURAH_COUNT,
  READER_MODE_VALUES,
  type Persisted,
  type ReaderCore,
  type ReaderMode,
} from "./reader-core.svelte";

const STORAGE_KEY = "easyquran.reader";

/**
 * Decode a raw localStorage blob into a validated `Partial<Persisted>`. Missing
 * or invalid fields are simply skipped so they keep their runtime default. A
 * blob whose explicit `v` is not the current version is discarded wholesale
 * (a future, incompatible shape) — but a blob with NO `v` (legacy data from
 * before the version field existed) is migrated forward; the next write
 * re-stamps the current version.
 */
export function decodeReader(raw: unknown): Partial<Persisted> {
  if (isFutureSchema(raw, READER_SCHEMA_VERSION)) return {};
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

  const lrKind = asNullableObject(stored.lastRead);
  if (lrKind === "null") {
    out.lastRead = null;
  } else if (lrKind === "object") {
    const lr = asObject(stored.lastRead)!;
    const num = asNumber(lr.num, -Infinity, Infinity);
    const n = asNumber(lr.n, -Infinity, Infinity);
    if (num !== undefined && n !== undefined) out.lastRead = { num, n };
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
}

export interface ReaderPersistence {
  hydrate(): void;
  writeNow(): void;
  scheduleNoteWrite(): void;
  flushNoteWrite(): void;
  readonly hydrated: boolean;
  dispose(): void;
}

export function createReaderPersistence(core: ReaderCore): ReaderPersistence {
  const noteWriter = trailingDebounce(() => writeBlob(), NOTE_PERSIST_DEBOUNCE_MS);
  let hydrated = false;
  let teardowns: Array<() => void> = [];

  function writeBlob(): void {
    const { v, current, fontSize, mode, bookmarks, notes, lastRead } = core.s;
    writeJSON(STORAGE_KEY, { v, current, fontSize, mode, bookmarks, notes, lastRead });
  }

  return {
    get hydrated() {
      return hydrated;
    },
    hydrate() {
      if (hydrated) return;
      hydrated = true;
      applyPersisted(core.s, decodeReader(readJSON(STORAGE_KEY)));
      // Cross-tab: another tab's write re-syncs this tab. The 'storage' event
      // only fires in OTHER tabs, so our own writes never echo back here.
      teardowns.push(
        onStorageKey(STORAGE_KEY, () =>
          applyPersisted(core.s, decodeReader(readJSON(STORAGE_KEY))),
        ),
      );
      teardowns.push(onPageHide(() => noteWriter.flush()));
    },
    writeNow() {
      // An immediate write already captures the latest notes, so cancel any
      // pending debounced write rather than fire a redundant one shortly after.
      noteWriter.cancel();
      writeBlob();
    },
    scheduleNoteWrite() {
      noteWriter.schedule();
    },
    flushNoteWrite() {
      noteWriter.flush();
    },
    dispose() {
      noteWriter.flush();
      for (const teardown of teardowns) teardown();
      teardowns = [];
    },
  };
}
