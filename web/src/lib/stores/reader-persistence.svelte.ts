import { type AuthStatus, authState } from "#lib/auth/auth-state.svelte.js";
import {
  ARABIC_FONT_IDS,
  type ArabicFontId,
  type TranslationFamily,
} from "#lib/config/reader-fonts.js";
import type { QuranSourceId } from "#lib/data/quran-types.js";
import type { VerseKey } from "#lib/data/quran.js";
import { clientQuranEnvironment } from "#lib/quran/client-environment.js";
import { isScriptEnabled } from "#lib/quran/script-availability.js";
import { asNumberRecord } from "#lib/storage/decoders.js";
import {
  asArray,
  asBooleanRecord,
  asLiteral,
  asNumber,
  asObject,
  asString,
  asStringRecord,
  onPageHide,
  onStorageKey,
  readJSON,
  readRaw,
  trailingDebounce,
  writeJSON,
  writeRaw,
} from "#lib/storage/index.js";

import {
  ARABIC_FONT_MAX,
  ARABIC_FONT_MIN,
  ARABIC_SCRIPT_VALUES,
  NOTE_PERSIST_DEBOUNCE_MS,
  SURAH_COUNT,
  READER_MODE_VALUES,
  TRANSLATION_FAMILY_VALUES,
  TRANSLATION_FONT_MAX,
  TRANSLATION_FONT_MIN,
  type Persisted,
  type ReaderCore,
  type ReaderMode,
  type RecentsEntry,
  type SurahProgress,
} from "./reader-core.svelte";
import { applyReaderPresentation } from "./reader-presentation";

const STORAGE_KEY = "easyquran.reader";
const ANCHOR_PERSIST_DEBOUNCE_MS = 300;

// Notes live outside the reader blob under a per-owner key (EC-17): owner = user id
// when signed in, `local` otherwise, so one account's notes are never read for another.
const NOTES_SCHEMA_VERSION = 1;
const LOCAL_NOTES_OWNER = "local";
const LEGACY_NOTES_SEEDED_KEY = "eq.notes.legacy-seeded";

function notesKeyFor(owner: string): string {
  return `easyquran.notes.${owner}`;
}

export interface NotesEnvelope {
  v: number;
  notes: Record<VerseKey, string>;
}

/**
 * Auth surface the notes owner is resolved from. Structural on purpose: the real
 * `authState` satisfies it and tests can pass a plain stub.
 */
export interface NotesOwnerSource {
  readonly status: AuthStatus;
  readonly user: { readonly id: number } | null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser (asObject/asStringRecord validate every field)
export function decodeNotes(raw: unknown): NotesEnvelope | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  if (stored.v !== NOTES_SCHEMA_VERSION) return undefined;
  return { v: NOTES_SCHEMA_VERSION, notes: asStringRecord(asObject(stored.notes) ?? {}) };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser (asObject/asNumber/asLiteral validate every field)
export function decodeReader(raw: unknown): Partial<Persisted> {
  const stored = asObject(raw);
  if (!stored) return {};

  if (stored.v !== undefined && !asNumber(stored.v, 1, Number.POSITIVE_INFINITY)) return {};

  const out: Partial<Persisted> = {};

  const current = asNumber(stored.current, 1, SURAH_COUNT);
  if (current !== undefined) out.current = current;

  const fontSize = asNumber(stored.fontSize, ARABIC_FONT_MIN, ARABIC_FONT_MAX);
  if (fontSize !== undefined) out.fontSize = fontSize;

  const arabicFont = asLiteral<ArabicFontId>(stored.arabicFont, ARABIC_FONT_IDS);
  if (arabicFont) out.arabicFont = arabicFont;

  // v4 field: absent in v3 blobs → default (uthmani) from READER_DEFAULTS.
  const arabicScript = asLiteral<QuranSourceId>(stored.arabicScript, ARABIC_SCRIPT_VALUES);
  if (arabicScript) out.arabicScript = arabicScript;

  const translationSize = asNumber(
    stored.translationSize,
    TRANSLATION_FONT_MIN,
    TRANSLATION_FONT_MAX,
  );
  if (translationSize !== undefined) out.translationSize = translationSize;

  // v5 field: absent in v≤4 blobs → default ("sans") from READER_DEFAULTS.
  const translationFamily = asLiteral<TranslationFamily>(
    stored.translationFamily,
    TRANSLATION_FAMILY_VALUES,
  );
  if (translationFamily) out.translationFamily = translationFamily;

  const mode = asLiteral<ReaderMode>(stored.mode, READER_MODE_VALUES);
  if (mode) out.mode = mode;

  const bookmarksObj = asObject(stored.bookmarks);
  if (bookmarksObj) out.bookmarks = asBooleanRecord(bookmarksObj);

  // v5 field: absent in v≤4 blobs → legacy bookmarks carry no known save time (EC-46).
  const savedAtObj = asObject(stored.bookmarkSavedAt);
  if (savedAtObj) out.bookmarkSavedAt = asNumberRecord(savedAtObj, 0, Number.MAX_SAFE_INTEGER);

  // Legacy v≤4 field: decode-only migration input for the per-owner notes key;
  // v5 never writes it back into the blob.
  const notesObj = asObject(stored.notes);
  if (notesObj) out.notes = asStringRecord(notesObj);

  if (stored.lastRead === null) {
    out.lastRead = null;
  } else {
    const lr = asObject(stored.lastRead);
    if (lr) {
      const num = asNumber(lr.num, -Infinity, Infinity);
      const n = asNumber(lr.n, -Infinity, Infinity);
      const sourceId = asString(lr.sourceId);
      if (num !== undefined && n !== undefined)
        out.lastRead = sourceId !== undefined ? { num, n, sourceId } : { num, n };
    }
  }

  if (stored.lastReadAnchor === null) {
    out.lastReadAnchor = null;
  } else {
    const a = asObject(stored.lastReadAnchor);
    if (a) {
      const verseKey = asString(a.verseKey);
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
      const sourceId = asString(e.sourceId);
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
      if (furthestAyah !== undefined && ts !== undefined) progress[num] = { furthestAyah, ts };
    }
    out.progress = progress;
  }

  return out;
}

function applyPresentation(s: ReaderCore["s"]): void {
  applyReaderPresentation(s.mode, s.fontSize, s.arabicFont, s.translationSize, s.translationFamily);
}

function applyPersisted(s: ReaderCore["s"], p: Partial<Persisted>): void {
  if (p.current !== undefined) s.current = p.current;
  if (p.fontSize !== undefined) s.fontSize = p.fontSize;
  if (p.arabicFont !== undefined) s.arabicFont = p.arabicFont;
  // A production-disabled persisted script falls back to the default; the stored
  // preference is left intact so local/dev still hydrates it.
  if (p.arabicScript !== undefined && isScriptEnabled(p.arabicScript, clientQuranEnvironment())) {
    s.arabicScript = p.arabicScript;
  }
  if (p.translationSize !== undefined) s.translationSize = p.translationSize;
  if (p.translationFamily !== undefined) s.translationFamily = p.translationFamily;
  if (p.mode !== undefined) s.mode = p.mode;
  if (p.bookmarks !== undefined) s.bookmarks = p.bookmarks;
  if (p.bookmarkSavedAt !== undefined) s.bookmarkSavedAt = p.bookmarkSavedAt;
  // `p.notes` (legacy blob notes) is deliberately NOT applied here: notes state is
  // owned by the per-owner notes key loader below, which seeds from it exactly once.
  if (p.lastRead !== undefined) s.lastRead = p.lastRead;
  if (p.lastReadAnchor !== undefined) s.lastReadAnchor = p.lastReadAnchor;
  if (p.recents !== undefined) s.recents = p.recents;
  if (p.progress !== undefined) s.progress = p.progress;
}

export interface ReaderPersistence {
  hydrate(modeOverride?: ReaderMode): void;
  writeNow(): void;
  scheduleNoteWrite(): void;
  flushNoteWrite(): void;
  scheduleAnchorWrite(): void;
  flushAnchorWrite(): void;
  /**
   * Re-resolves the notes owner (post-auth user id, else `local`) and swaps the
   * in-memory notes when it changed. No-op while auth is unresolved or before
   * hydrate; cheap enough to call from every notes access path (EC-17).
   */
  syncNotesOwner(): void;
  readonly hydrated: boolean;
  dispose(): void;
}

export function createReaderPersistence(
  core: ReaderCore,
  auth: NotesOwnerSource = authState,
): ReaderPersistence {
  const noteWriter = trailingDebounce(() => writeNotes(), NOTE_PERSIST_DEBOUNCE_MS);
  const anchorWriter = trailingDebounce(() => writeBlob(), ANCHOR_PERSIST_DEBOUNCE_MS);
  let hydrated = false;
  let dirty = false;
  let teardowns: Array<() => void> = [];
  // Notes owner: null until auth resolves (deferred first load), then "local" or
  // the signed-in user id. `legacyNotes` is the v≤4 blob decode captured at hydrate.
  let notesOwner: string | null = null;
  let legacyNotes: Record<VerseKey, string> | undefined;
  let notesListener: (() => void) | null = null;

  function writeBlob(): void {
    if (!hydrated) {
      dirty = true;
      return;
    }
    const {
      v,
      current,
      fontSize,
      arabicFont,
      arabicScript,
      translationSize,
      translationFamily,
      mode,
      bookmarks,
      bookmarkSavedAt,
      lastRead,
      lastReadAnchor,
      recents,
      progress,
    } = core.s;
    writeJSON(STORAGE_KEY, {
      v,
      current,
      fontSize,
      arabicFont,
      arabicScript,
      translationSize,
      translationFamily,
      mode,
      bookmarks,
      bookmarkSavedAt,
      lastRead,
      lastReadAnchor,
      recents,
      progress,
    });
  }

  function writeNotes(): void {
    if (notesOwner === null) return;
    writeJSON(notesKeyFor(notesOwner), { v: NOTES_SCHEMA_VERSION, notes: core.s.notes });
  }

  function listenForNotes(owner: string): void {
    notesListener?.();
    notesListener = onStorageKey(notesKeyFor(owner), () => {
      if (notesOwner === null) return;
      const envelope = decodeNotes(readJSON(notesKeyFor(notesOwner)));
      if (envelope) core.s.notes = envelope.notes;
    });
  }

  function loadNotes(legacy: Record<VerseKey, string> | undefined): void {
    if (notesOwner === null) return;
    const key = notesKeyFor(notesOwner);
    const envelope = decodeNotes(readJSON(key));
    if (envelope) {
      core.s.notes = envelope.notes;
      listenForNotes(notesOwner);
      return;
    }
    // One-time migration: seed the owner's key from the legacy v≤4 blob notes.
    // The marker keeps a cleared key from ever re-seeding (the blob is never deleted).
    if (
      legacy !== undefined &&
      Object.keys(legacy).length > 0 &&
      readRaw("local", LEGACY_NOTES_SEEDED_KEY) === null
    ) {
      core.s.notes = { ...legacy };
      writeJSON(key, { v: NOTES_SCHEMA_VERSION, notes: core.s.notes });
      writeRaw("local", LEGACY_NOTES_SEEDED_KEY, "1");
      listenForNotes(notesOwner);
      return;
    }
    core.s.notes = {};
    listenForNotes(notesOwner);
  }

  function resolveOwner(): string | null {
    // Defer while auth is unresolved: legacy notes must attach to the owner signed
    // in at the first post-auth hydrate, never blindly to `local`.
    if (auth.status === "unknown") return null;
    if (auth.status === "authenticated" && auth.user !== null) return String(auth.user.id);
    return LOCAL_NOTES_OWNER;
  }

  function syncNotesOwner(): void {
    if (!hydrated) return;
    const next = resolveOwner();
    if (next === null || next === notesOwner) return;
    if (notesOwner === null) {
      notesOwner = next;
      // Notes written before auth resolved belong to this device session — merge
      // them over the loaded state so they survive the first load. With nothing
      // in memory, loadNotes above (or the seed inside it) already settled the key.
      const preAuth = { ...core.s.notes };
      loadNotes(legacyNotes);
      if (Object.keys(preAuth).length > 0) {
        for (const [k, v] of Object.entries(preAuth)) {
          // SAFETY: preAuth was spread from core.s.notes (Record<VerseKey, string>), so every key is already a VerseKey.
          core.s.notes[k as VerseKey] = v;
        }
        writeNotes();
      }
      return;
    }
    // Owner switch (login/logout): park the outgoing owner's notes on its key,
    // then replace memory with the incoming owner's — never merge across owners.
    writeNotes();
    noteWriter.cancel();
    notesOwner = next;
    loadNotes(undefined);
  }

  return {
    get hydrated() {
      return hydrated;
    },
    hydrate(modeOverride?: ReaderMode) {
      if (hydrated) return;
      hydrated = true;
      const stored = decodeReader(readJSON(STORAGE_KEY));
      legacyNotes = stored.notes;
      if (dirty) stored.current = undefined;
      applyPersisted(core.s, stored);
      const adoptedMode = modeOverride && modeOverride !== core.s.mode ? modeOverride : null;
      if (adoptedMode) core.s.mode = adoptedMode;
      applyPresentation(core.s);
      teardowns.push(
        onStorageKey(STORAGE_KEY, () => {
          applyPersisted(core.s, decodeReader(readJSON(STORAGE_KEY)));
          applyPresentation(core.s);
        }),
      );
      teardowns.push(
        onPageHide(() => {
          noteWriter.flush();
          anchorWriter.flush();
        }),
      );
      syncNotesOwner();
      if (dirty || adoptedMode) {
        dirty = false;
        writeBlob();
      }
    },
    writeNow() {
      syncNotesOwner();
      // Flush (not cancel) a pending note write: notes live in their own key now,
      // so dropping the debounced write would silently lose the edit.
      noteWriter.flush();
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
    syncNotesOwner,
    dispose() {
      noteWriter.flush();
      anchorWriter.flush();
      notesListener?.();
      notesListener = null;
      for (const teardown of teardowns) teardown();
      teardowns = [];
    },
  };
}
