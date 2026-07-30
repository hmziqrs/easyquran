/* ════════════════════════════════════════════════════════════════════════
   reader.svelte.ts — the reading experience state.

   A single Svelte 5 runes class, SSR-safe (guards every DOM/localStorage
   access behind `browser`). Persists the durable slice (current surah, font
   size, bookmarks, notes, last-read) to localStorage under its own key,
   separate from appearance prefs. Recitation audio is not implemented yet —
   the listening UI was removed; when real audio arrives it will be built
   against the live API, not a simulated surface.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { SURAHS, surahByNum, parseKey, type VerseKey } from "$lib/data/quran";

const STORAGE_KEY = "easyquran.reader";

export type BrowseMode = "surah" | "verse" | "juz" | "page";
export type ReaderMode = "verse" | "reading";

interface Persisted {
  current: number;
  fontSize: number;
  mode: ReaderMode;
  bookmarks: Record<VerseKey, boolean>;
  notes: Record<VerseKey, string>;
  lastRead: { num: number; n: number } | null;
}

interface ReaderState extends Persisted {
  query: string;
  browse: BrowseMode;
  openNote: VerseKey | null;
}

const DEFAULTS: ReaderState = {
  current: 1,
  fontSize: 33,
  mode: "verse",
  bookmarks: {},
  notes: {},
  lastRead: null,
  query: "",
  browse: "surah",
  openNote: null,
};

function load(): Partial<Persisted> {
  if (!browser) return {};
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") as Partial<Persisted>;
  } catch {
    return {};
  }
}

class ReaderStore {
  // SSR renders from DEFAULTS; saved state is pulled in after mount via
  // hydrate() so the prerendered HTML and the first client render agree.
  #s = $state<ReaderState>({ ...DEFAULTS });
  #hydrated = false;
  /** Per-open-surah synchronous verse cache — keeps copyVerse / bookmark text
   *  working without a Worker round-trip (doc §6.3). Seeded from prerendered
   *  page.data and refreshed from the sqlite-wasm Worker. */
  #versesBySurah = new Map<number, string[]>();
  /** Monotonic guard: bumped on every navigation so a stale Worker response for
   *  a previously-open surah can never clobber the currently-selected one. */
  #navToken = 0;

  /** Hydrate saved state from localStorage after mount (see note above). */
  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    const s = load();
    if (s.current != null) this.#s.current = s.current;
    if (s.fontSize != null) this.#s.fontSize = s.fontSize;
    if (s.mode) this.#s.mode = s.mode;
    if (s.bookmarks) this.#s.bookmarks = s.bookmarks;
    if (s.notes) this.#s.notes = s.notes;
    if (s.lastRead !== undefined) this.#s.lastRead = s.lastRead ?? null;
  }

  // ── persistence ──────────────────────────────────────────────────────
  private persist(): void {
    if (!browser) return;
    const { current, fontSize, mode, bookmarks, notes, lastRead } = this.#s;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ current, fontSize, mode, bookmarks, notes, lastRead }),
      );
    } catch {
      /* storage may be unavailable (private mode, quota) — non-fatal */
    }
  }

  // ── current surah & navigation ───────────────────────────────────────
  get current(): number {
    return this.#s.current;
  }
  get surah() {
    return surahByNum(this.#s.current);
  }
  setCurrent(num: number): void {
    this.#s.current = num;
    this.#navToken++; // invalidate any in-flight Worker refresh for the prior surah
    this.#s.query = "";
    this.#s.openNote = null;
    this.persist();
  }

  /** Jump to a specific verse (from search / bookmarks / continue-reading). */
  openVerse(num: number, n: number): void {
    this.#s.current = num;
    this.#navToken++;
    this.#s.query = "";
    this.#s.browse = "surah";
    this.#s.openNote = null;
    this.#s.lastRead = { num, n };
    this.persist();
    if (browser) window.scrollTo(0, 0);
  }

  // ── search ───────────────────────────────────────────────────────────
  get query(): string {
    return this.#s.query;
  }
  get hasQuery(): boolean {
    return this.#s.query.trim().length > 0;
  }
  setQuery(v: string): void {
    this.#s.query = v;
  }
  clearQuery(): void {
    this.#s.query = "";
  }

  // ── sidebar browse mode (Surah / Verse / Juz / Page) ─────────────────
  get browseMode(): BrowseMode {
    return this.#s.browse;
  }
  get browseSurah(): boolean {
    return this.#s.browse === "surah";
  }
  get browseVerse(): boolean {
    return this.#s.browse === "verse";
  }
  get browseJuz(): boolean {
    return this.#s.browse === "juz";
  }
  get browsePage(): boolean {
    return this.#s.browse === "page";
  }
  setBrowse(browse: BrowseMode): void {
    this.#s.browse = browse;
  }

  // ── font size ────────────────────────────────────────────────────────
  get fontSize(): number {
    return this.#s.fontSize;
  }
  get arabicSizePx(): string {
    return `${this.#s.fontSize}px`;
  }
  bigger(): void {
    this.#s.fontSize = Math.min(56, this.#s.fontSize + 3);
    this.persist();
  }
  smaller(): void {
    this.#s.fontSize = Math.max(22, this.#s.fontSize - 3);
    this.persist();
  }

  // ── reading mode (verse-by-verse vs continuous mushaf text) ──────────
  get mode(): ReaderMode {
    return this.#s.mode;
  }
  get isVerseMode(): boolean {
    return this.#s.mode === "verse";
  }
  get isReadingMode(): boolean {
    return this.#s.mode === "reading";
  }
  setMode(mode: ReaderMode): void {
    if (this.#s.mode === mode) return;
    this.#s.mode = mode;
    this.persist();
  }

  // ── bookmarks ────────────────────────────────────────────────────────
  isBookmarked(key: VerseKey): boolean {
    return !!this.#s.bookmarks[key];
  }
  toggleBookmark(key: VerseKey): void {
    this.#s.bookmarks = { ...this.#s.bookmarks };
    if (this.#s.bookmarks[key]) delete this.#s.bookmarks[key];
    else this.#s.bookmarks[key] = true;
    this.persist();
  }
  /** Bookmarks as an ordered list (surah order, then ayah). */
  get bookmarkList(): { key: VerseKey; ref: string; text: string; num: number; n: number }[] {
    return Object.keys(this.#s.bookmarks
      ? this.#s.bookmarks
      : ({} as Record<VerseKey, boolean>))
      .filter((k) => this.#s.bookmarks[k])
      .map((k) => {
        const { num, n } = parseKey(k);
        const s = surahByNum(num);
        return {
          key: k,
          ref: `${s.name} ${num}:${n}`,
          text: this.versesFor(num)[n - 1] ?? "",
          num,
          n,
        };
      })
      .sort((a, b) => a.num - b.num || a.n - b.n);
  }
  get bookmarkCount(): number {
    return this.bookmarkList.length;
  }

  // ── notes ────────────────────────────────────────────────────────────
  getNote(key: VerseKey): string {
    return this.#s.notes[key] ?? "";
  }
  setNote(key: VerseKey, v: string): void {
    this.#s.notes = { ...this.#s.notes, [key]: v };
    this.persist();
  }
  get openNote(): VerseKey | null {
    return this.#s.openNote;
  }
  toggleNote(key: VerseKey): void {
    this.#s.openNote = this.#s.openNote === key ? null : key;
  }

  // ── last read ────────────────────────────────────────────────────────
  get lastRead() {
    return this.#s.lastRead;
  }
  get hasLastRead(): boolean {
    return this.#s.lastRead !== null;
  }
  get lastReadRef(): string {
    const lr = this.#s.lastRead;
    if (!lr) return "";
    return `${surahByNum(lr.num).name} ${lr.num}:${lr.n}`;
  }

  // ── synchronous verse cache (doc §6.3) ───────────────────────────────
  /** Synchronous verse text for a surah from the open-surah cache (or "" if the
   *  surah hasn't been seeded yet this session). */
  versesFor(num: number): string[] {
    return this.#versesBySurah.get(num) ?? [];
  }
  /** Seed a surah's verses into the sync cache (from prerendered page.data). */
  seedSurah(num: number, verses: string[]): void {
    if (verses.length) this.#versesBySurah.set(num, verses);
  }

  /**
   * Best-effort refresh of a surah's verses from the sqlite-wasm Worker, guarded
   * by #navToken so a response for a previously-open surah is discarded. No-op
   * until the Worker is ready; never throws (the prerendered sync cache already
   * serves the open surah, so failure is silently absorbed).
   */
  async refreshFromWorker(num: number): Promise<void> {
    if (!browser) return;
    const token = this.#navToken;
    try {
      const { quranWorker } = await import("$lib/quran/worker-client");
      if (!quranWorker.ready) return;
      const verses = await quranWorker.readSurah(num);
      if (token !== this.#navToken) return; // a navigation happened since
      if (num !== this.#s.current) return; // user moved on
      if (verses && verses.length) this.seedSurah(num, verses);
    } catch {
      /* prerender/cache still serves the surah */
    }
  }

  // ── verse text (for copy/share) ──────────────────────────────────────
  private verseText(key: VerseKey): string {
    const { num, n } = parseKey(key);
    return this.versesFor(num)[n - 1] ?? "";
  }

  // ── verse actions: copy & share ──────────────────────────────────────
  /** Copy a verse (Arabic + ref) to the clipboard. Returns success. */
  async copyVerse(key: VerseKey): Promise<boolean> {
    if (!browser) return false;
    const { num, n } = parseKey(key);
    const ref = `${surahByNum(num).name} ${num}:${n}`;
    try {
      await navigator.clipboard.writeText(`${this.verseText(key)}\n${ref}`);
      return true;
    } catch {
      return false;
    }
  }

  /** Share a verse via the Web Share API when available, else fall back to
   *  copying. Returns what happened. */
  async shareVerse(key: VerseKey): Promise<"shared" | "copied" | "failed"> {
    if (!browser) return "failed";
    const { num, n } = parseKey(key);
    const ref = `${surahByNum(num).name} ${num}:${n}`;
    const text = `${this.verseText(key)}\n${ref}`;
    try {
      if (navigator.share) {
        await navigator.share({ title: ref, text });
        return "shared";
      }
      await navigator.clipboard.writeText(text);
      return "copied";
    } catch {
      return "failed";
    }
  }

  /** Total surah count (for the sidebar list source). */
  get surahCount(): number {
    return SURAHS.length;
  }
}

export const reader = new ReaderStore();
