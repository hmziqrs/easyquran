import { parseVerseKey } from "#lib/bookmarks/schema.js";
import type { VerseKey } from "#lib/data/quran.js";
import { asObject, asStringRecord, onStorageKey, readJSON } from "#lib/storage/index.js";

/**
 * Notes widget source (Feature 5.8 / BM-03). Notes are local-only and per-owner
 * (`easyquran.notes.{owner}`); the reader persistence layer owns every write
 * (reader-persistence.svelte.ts). This reads the same key so the widget can
 * enumerate notes without widening the reader store API. Owner resolution
 * mirrors persistence: signed-in user id, else `local` (EC-17 — never another
 * owner's notes).
 */

const NOTES_SCHEMA_VERSION = 1;
const LOCAL_NOTES_OWNER = "local";

function notesKeyFor(owner: string): string {
  return `easyquran.notes.${owner}`;
}

export interface NoteRow {
  readonly key: VerseKey;
  readonly surah: number;
  readonly ayah: number;
  readonly text: string;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed `easyquran.notes.{owner}` localStorage envelope; this decoder is its parse boundary.
function decodeRows(raw: unknown): readonly NoteRow[] {
  const stored = asObject(raw);
  if (!stored || stored.v !== NOTES_SCHEMA_VERSION) return [];
  const rows: NoteRow[] = [];
  const notes = asStringRecord(asObject(stored.notes) ?? {});
  for (const [key, text] of Object.entries(notes)) {
    const parsed = parseVerseKey(key);
    if (!parsed || text.length === 0) continue;
    // SAFETY: parseVerseKey only returns for strings matching the VerseKey `s:a` shape, so key is a valid VerseKey.
    rows.push({ key: key as VerseKey, surah: parsed.surah, ayah: parsed.ayah, text });
  }
  // Notes carry no timestamps (plain strings) — deterministic mushaf order.
  return rows.sort((a, b) => a.surah - b.surah || a.ayah - b.ayah);
}

class NotesSource {
  #rows = $state.raw<readonly NoteRow[]>([]);
  #owner: string | null = null;
  #listener: (() => void) | null = null;

  /** Swaps the watched owner (auth transitions); re-reads and re-listens. */
  setOwner(owner: string): void {
    if (this.#owner === owner) return;
    this.#owner = owner;
    this.#listener?.();
    this.#listener = onStorageKey(notesKeyFor(owner), () => {
      this.#rows = decodeRows(readJSON(notesKeyFor(owner)));
    });
    this.#rows = decodeRows(readJSON(notesKeyFor(owner)));
  }

  get rows(): readonly NoteRow[] {
    return this.#rows;
  }

  dispose(): void {
    this.#listener?.();
    this.#listener = null;
    this.#owner = null;
    this.#rows = [];
  }
}

export const notesSource = new NotesSource();
export { LOCAL_NOTES_OWNER };
