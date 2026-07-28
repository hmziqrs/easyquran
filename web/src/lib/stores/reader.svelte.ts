/* ════════════════════════════════════════════════════════════════════════
   reader.svelte.ts — the reading experience state.

   A single Svelte 5 runes class, SSR-safe (guards every DOM/localStorage
   access behind `browser`). Persists the durable slice (current surah, font
   size, bookmarks, notes, last-read) to localStorage under its own key,
   separate from appearance prefs. The player is simulated — recitation audio
   in the full app would drive the same progress/playing surface.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import {
  SURAHS,
  surahByNum,
  verseKey,
  parseKey,
  type VerseKey,
} from "$lib/data/quran";

const STORAGE_KEY = "easyquran.reader";
const RECITER = "Mishary Rashid Alafasy";

export type ReaderTab = "surahs" | "bookmarks";

interface Persisted {
  current: number;
  fontSize: number;
  bookmarks: Record<VerseKey, boolean>;
  notes: Record<VerseKey, string>;
  lastRead: { num: number; n: number } | null;
}

interface ReaderState extends Persisted {
  query: string;
  tab: ReaderTab;
  openNote: VerseKey | null;
  playing: VerseKey | null;
  progress: number; // 0..100
  paused: boolean;
  /** duration (s) of whatever is playing — derived from verse length. */
  total: number;
}

const DEFAULTS: ReaderState = {
  current: 1,
  fontSize: 33,
  bookmarks: {},
  notes: {},
  lastRead: null,
  query: "",
  tab: "surahs",
  openNote: null,
  playing: null,
  progress: 0,
  paused: false,
  total: 10,
};

function load(): Partial<Persisted> {
  if (!browser) return {};
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}") as Partial<Persisted>;
  } catch {
    return {};
  }
}

const fmtTime = (n: number): string =>
  `${Math.floor(n / 60)}:${String(Math.round(n % 60)).padStart(2, "0")}`;

class ReaderStore {
  // SSR renders from DEFAULTS; saved state is pulled in after mount via
  // hydrate() so the prerendered HTML and the first client render agree.
  #s = $state<ReaderState>({ ...DEFAULTS });
  #hydrated = false;
  #timer: ReturnType<typeof setInterval> | null = null;

  /** Hydrate saved state from localStorage after mount (see note above). */
  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    const s = load();
    if (s.current != null) this.#s.current = s.current;
    if (s.fontSize != null) this.#s.fontSize = s.fontSize;
    if (s.bookmarks) this.#s.bookmarks = s.bookmarks;
    if (s.notes) this.#s.notes = s.notes;
    if (s.lastRead !== undefined) this.#s.lastRead = s.lastRead ?? null;
  }

  // ── persistence ──────────────────────────────────────────────────────
  private persist(): void {
    if (!browser) return;
    const { current, fontSize, bookmarks, notes, lastRead } = this.#s;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ current, fontSize, bookmarks, notes, lastRead }),
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
    this.#s.query = "";
    this.#s.openNote = null;
    this.persist();
  }

  /** Jump to a specific verse (from search / bookmarks / continue-reading). */
  openVerse(num: number, n: number): void {
    this.#s.current = num;
    this.#s.query = "";
    this.#s.tab = "surahs";
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

  // ── sidebar tab ──────────────────────────────────────────────────────
  get tab(): ReaderTab {
    return this.#s.tab;
  }
  get isSurahTab(): boolean {
    return this.#s.tab === "surahs";
  }
  get isBookmarkTab(): boolean {
    return this.#s.tab === "bookmarks";
  }
  setTab(tab: ReaderTab): void {
    this.#s.tab = tab;
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
          text: s.verses[n - 1] ?? "",
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

  /** Is this verse the one currently playing? */
  isPlayingVerse(key: VerseKey): boolean {
    return this.#s.playing === key;
  }
  /** Soft highlight on the playing verse row. */
  rowHighlight(key: VerseKey): boolean {
    return this.#s.playing === key;
  }

  // ── simulated player ─────────────────────────────────────────────────
  private verseText(key: VerseKey): string {
    const { num, n } = parseKey(key);
    return surahByNum(num).verses[n - 1] ?? "";
  }
  private durationFor(key: VerseKey): number {
    return Math.max(6, Math.round(this.verseText(key).length / 6));
  }

  private tick(key: VerseKey, total: number, resume = false): void {
    this.stopTimer();
    this.#s.total = total;
    if (resume) {
      this.#s.paused = false;
    } else {
      this.#s.playing = key;
      this.#s.progress = 0;
      this.#s.paused = false;
    }
    this.#timer = setInterval(() => {
      const next = this.#s.progress + 100 / (total * 10);
      if (next >= 100) {
        this.#s.progress = 100;
        this.#s.paused = true;
        this.stopTimer();
      } else {
        this.#s.progress = next;
      }
    }, 100);
  }
  private stopTimer(): void {
    if (this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = null;
    }
  }

  playVerse(key: VerseKey): void {
    this.tick(key, this.durationFor(key));
    const { num, n } = parseKey(key);
    this.#s.lastRead = { num, n };
    this.persist();
  }
  playSurah(num: number): void {
    this.tick(verseKey(num, 1), 40);
  }
  togglePlay(): void {
    const key = this.#s.playing;
    if (!key) return;
    this.stopTimer();
    if (this.#s.progress >= 100) this.tick(key, this.#s.total);
    else if (this.#s.paused) this.tick(key, this.#s.total, true);
    else this.#s.paused = true;
  }
  stop(): void {
    this.stopTimer();
    this.#s.playing = null;
    this.#s.progress = 0;
    this.#s.paused = false;
  }

  get isPlaying(): boolean {
    return this.#s.playing !== null;
  }
  get isPaused(): boolean {
    return this.#s.paused;
  }
  get atEnd(): boolean {
    return this.#s.progress >= 100;
  }
  get nowPlayingRef(): string {
    const key = this.#s.playing;
    if (!key) return "";
    const { num, n } = parseKey(key);
    return `${surahByNum(num).name} ${num}:${n}`;
  }
  get reciter(): string {
    return RECITER;
  }
  get progressPct(): string {
    return `${this.#s.progress}%`;
  }
  get timeLabel(): string {
    const secs = Math.round((this.#s.total * this.#s.progress) / 100);
    return `${fmtTime(secs)} / ${fmtTime(this.#s.total)}`;
  }
  /** Total surah count (for the sidebar list source). */
  get surahCount(): number {
    return SURAHS.length;
  }
}

export const reader = new ReaderStore();
