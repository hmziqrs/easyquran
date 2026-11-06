import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { deviceIdFor } from "#lib/device/identity.js";
import {
  asObject,
  asString,
  onPageHide,
  readJSON,
  readRaw,
  removeJSON,
  trailingDebounce,
  tryWriteJSON,
  writeRaw,
} from "#lib/storage/index.js";
import { createClock, localClockStorage, type SyncClock } from "#lib/sync/clock.js";
import { isUuidV7, newEntityId, uuidV7FromLegacy } from "#lib/sync/ids.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  type RegisteredSyncDomain,
} from "#lib/sync/index.js";
import { mergedDirty } from "#lib/sync/round-status.js";
import { compareStamps, mergeThread, roundRatio, type JsonObject } from "#lib/sync/merge.js";

import { deviceKind } from "./device";
import {
  createSessionsDomain,
  sessionsDomainFor,
  type SessionsAuthLike,
  type SessionsAuthStateLike,
} from "./domain";
import { cameFromReader, consumeEntry, markEntry, type EntryIntent } from "./entry";
import {
  LOCAL_THREAD_CAP,
  byNewest,
  compareVerse,
  decodeArchivePage,
  decodeEnvelope,
  decodeThread,
  emptyEnvelope,
  isNewerStore,
  isOpen,
  parseVerseKey,
  type EndReason,
  type ReadingThread,
  type RowResult,
  type SessionOrigin,
  type SessionRound,
  type SessionWireRow,
  type ProgressBySurah,
  type SessionsEnvelope,
  type SurahProgress,
  type ThreadStamps,
  type ThreadTombstone,
} from "./schema";

/** Threads cleared by `clearHistory`, kept so Undo can put them back. */
export interface HistoryClear {
  readonly owner: string;
  readonly threads: readonly ReadingThread[];
}

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SessionsEngineLike {
  flush(domain?: string): Promise<void>;
  start(): () => void;
  setStorageDegraded?(on: boolean): void;
}

export interface SessionsStoreDeps {
  readonly engine?: SessionsEngineLike;
  readonly auth?: SessionsAuthLike & {
    get<T>(path: string): Promise<{ ok: boolean; data?: T | null }>;
  };
  readonly state?: SessionsAuthStateLike & { notifySessionLost?(): void };
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

/** What a reader route shows — the reader hands this to `enter()`. */
export interface ReaderTarget {
  /** The route's natural scope: `surah:73`, `page:574`, `juz:30`, `hizb:60`, `snippet:slug`. */
  readonly scope: string;
  /** First verse on screen, or the explicit ayah the URL names. */
  readonly surah: number;
  readonly ayah: number;
  /** The URL names an exact ayah (hash / `?v=`). */
  readonly exact: boolean;
  /** Bounded scope end (a snippet's last verse). */
  readonly boundEnd?: string | null;
  readonly sourceId: string | null;
}

export interface EnterResult {
  readonly thread: ReadingThread;
  /** Picked up an existing thread whose cursor is elsewhere — move the reader there. */
  readonly resumed: boolean;
}

export interface ArchiveView {
  readonly items: readonly ReadingThread[];
  readonly next: string | null;
  readonly loading: boolean;
  readonly loaded: boolean;
}

/**
 * Reading threads. Per-owner durable key
 * `easyquran.sessions.{owner}` (owner = user id when signed in, `local`
 * otherwise); the tab's current thread id lives in sessionStorage so a reload
 * continues it and two tabs read two threads. A thread the reader merely
 * opened stays an in-memory draft until something is actually read — bounces
 * never litter Continue. Signed in, the row is the queue: `dirty` persists with
 * the row, and a sync round sends the dirty rows, clearing the flag only when a
 * result acknowledges those exact stamps (contracts/sync.md). On sign-in the
 * device's anonymous threads fold into the account silently.
 */

const LOCAL_OWNER = "local";
const TAB_KEY = "easyquran.session.tab";
const SYNC_PAGE = 200;
/** A cached account nobody signed into for this long loses its non-dirty rows. */
const IDLE_ACCOUNT_MS = 180 * 24 * 60 * 60 * 1000;
/** Cursor moves coalesce: scrolling marks many verses in seconds. */
const PUSH_DEBOUNCE_MS = 4000;
const PERSIST_DEBOUNCE_MS = 400;
const ARCHIVE_PAGE = 20;
/** Bounds the whole-history walk: 500 pages of 20 is 10,000 threads. */
const MAX_ARCHIVE_PAGES = 500;

/**
 * Undo writes a thread back newer than the tombstone it replaces: a signed-out row
 * with no stamps merges by `updatedAt`, so an equal or older time would keep the delete.
 */
function restoreStamp(tombstone: ReadingThread | undefined): string {
  const after = tombstone === undefined ? 0 : Date.parse(tombstone.updatedAt) + 1;
  return new Date(Math.max(Date.now(), after)).toISOString();
}

function keyFor(owner: string): string {
  return `easyquran.sessions.${owner}`;
}

function nowIso(): string {
  return new Date().toISOString();
}


function registersOf(thread: ReadingThread): JsonObject {
  return {
    id: thread.id,
    identity: {
      stamp: thread.identityStamp,
      value: {
        origin: thread.origin,
        originRef: thread.originRef,
        scope: thread.scope,
        boundEnd: thread.boundEnd,
        startedAt: thread.startedAt,
      },
    },
    cursor: {
      stamp: thread.cursorStamp,
      value: {
        surah: thread.surah,
        ayah: thread.ayah,
        localPage: thread.localPage,
        ratio: thread.ratio,
        sourceId: thread.sourceId,
        device: thread.device,
        deviceId: thread.deviceId,
      },
    },
    lifecycle: {
      stamp: thread.lifecycleStamp,
      value: { state: thread.state, endedAt: thread.endedAt, endReason: thread.endReason },
    },
  };
}

function stampsOf(row: ThreadStamps): readonly (string | null)[] {
  return [row.identityStamp, row.cursorStamp, row.lifecycleStamp];
}

/** S1 merge on the local row shape. `mergedDirty` holds the rule for the flag. */
function mergeRow(a: ReadingThread, b: ReadingThread): ReadingThread {
  // Signed-out rows carry no stamps yet: their own time decides.
  if (a.cursorStamp === null && b.cursorStamp === null) return b.updatedAt > a.updatedAt ? b : a;
  const merged = decodeMerged(mergeThread(registersOf(a), registersOf(b)));
  const dirty = mergedDirty(
    { dirty: a.dirty, stamps: stampsOf(a) },
    { dirty: b.dirty, stamps: stampsOf(b) },
    [merged.identityStamp ?? null, merged.cursorStamp ?? null, merged.lifecycleStamp ?? null],
  );
  return {
    ...a,
    ...merged,
    updatedAt: a.updatedAt > b.updatedAt ? a.updatedAt : b.updatedAt,
    dirty,
    rejected: a.rejected && b.rejected,
    extras: { ...a.extras, ...b.extras },
  };
}

function decodeMerged(merged: JsonObject): Partial<ReadingThread> {
  const stampOf = (name: string): string | null => asString(asObject(merged[name])?.stamp) ?? null;
  const fields = (name: string): JsonObject => asObject(asObject(merged[name])?.value) ?? {};
  const row = decodeThread({
    id: merged.id,
    ...fields("identity"),
    ...fields("cursor"),
    ...fields("lifecycle"),
    updatedAt: "1970-01-01T00:00:00.000Z",
    identityStamp: stampOf("identity"),
    cursorStamp: stampOf("cursor"),
    lifecycleStamp: stampOf("lifecycle"),
  });
  if (row === null) return {};
  const { updatedAt: _updatedAt, dirty: _dirty, rejected: _rejected, extras: _extras, ...rest } = row;
  return rest;
}

/**
 * Union by id through the merge core. Open threads are kept before the cap trims
 * history, and a dirty row is never evicted: it is the queue.
 */
function mergeThreads(
  a: readonly ReadingThread[],
  b: readonly ReadingThread[],
  now: number = Date.now(),
): ReadingThread[] {
  const byId = new Map<string, ReadingThread>();
  for (const row of [...a, ...b]) {
    const prev = byId.get(row.id);
    byId.set(row.id, prev === undefined ? row : mergeRow(prev, row));
  }
  const all = [...byId.values()].sort(byNewest);
  const pinned = all.filter((t) => t.dirty || isOpen(t, now));
  const rest = all.filter((t) => !t.dirty && !isOpen(t, now));
  return [...pinned, ...rest].slice(0, Math.max(LOCAL_THREAD_CAP, pinned.length)).sort(byNewest);
}

function mergeProgress(a: ProgressBySurah, b: ProgressBySurah): ProgressBySurah {
  const out = { ...a };
  for (const [key, row] of Object.entries(b)) {
    const surah = Number(key);
    const prev = out[surah];
    if (prev === undefined || row.furthestAyah > prev.furthestAyah) out[surah] = row;
    else if (row.dirty === true && row.furthestAyah === prev.furthestAyah && prev.dirty !== true) {
      out[surah] = row;
    }
  }
  return out;
}

function wireOf(thread: ReadingThread): SessionWireRow {
  const { dirty: _dirty, rejected: _rejected, extras, ...row } = thread;
  return { kind: "session.upsert", ...extras, ...row };
}

/** A `range:S:A-S:B` scope bounds itself at `S:B`. */
function boundFromScope(scope: string): string | null {
  const match = /^range:\d{1,3}:\d{1,3}-(\d{1,3}:\d{1,3})$/u.exec(scope);
  return match?.[1] ?? null;
}

/** Start verse of a `range:` scope, else null. */
function rangeStart(scope: string): { surah: number; ayah: number } | null {
  const match = /^range:(\d{1,3}:\d{1,3})-/u.exec(scope);
  return match?.[1] !== undefined ? parseVerseKey(match[1]) : null;
}

/** The sessionStorage tab record, or undefined when absent / unreadable. */
function parseTabRecord(raw: string | null) {
  if (raw === null || raw.length === 0) return undefined;
  try {
    return asObject(JSON.parse(raw));
  } catch {
    return undefined;
  }
}

function originOf(intent: EntryIntent | null): SessionOrigin {
  if (intent === null || intent.origin === "resume") return "link";
  return intent.origin;
}

export class SessionsStore {
  #owner = LOCAL_OWNER;
  #hydrated = false;
  #env = $state.raw<SessionsEnvelope>(emptyEnvelope());
  /** The tab's thread before anything was read in it — never persisted or synced. */
  #draft = $state.raw<ReadingThread | null>(null);
  #tabId = $state<string | null>(null);
  #tabScope: string | null = null;
  /** A whole-scope entry resumed this thread: offer "Start over". */
  #offer = $state<string | null>(null);
  #archive = $state.raw<ArchiveView>({ items: [], next: null, loading: false, loaded: false });
  #rejected = $state(0);
  #capped = $state(0);
  #updateRequired = $state(false);
  #storageFailed = $state(false);
  /** A store newer than this build: left untouched, and sync stays paused. */
  #futureStore = false;
  #clock: SyncClock | null = null;
  /** Past the tombstone floor: ids seen in this full pull. Others are gone. */
  #fullPullSeen: Set<string> | null = null;
  #cappedIds = new Set<string>();
  #registered: string | null = null;
  #device = deviceKind();
  #deviceId: string | null = null;

  readonly #engine: SessionsEngineLike;
  readonly #auth: NonNullable<SessionsStoreDeps["auth"]>;
  readonly #state: SessionsAuthStateLike & { notifySessionLost?(): void };
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;

  #persist = trailingDebounce(() => this.#writeNow(), PERSIST_DEBOUNCE_MS);
  #schedulePush = trailingDebounce(() => this.#push(), PUSH_DEBOUNCE_MS);

  constructor(deps: SessionsStoreDeps = {}) {
    this.#engine = deps.engine ?? syncEngine;
    this.#auth = deps.auth ?? authClient;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
    this.#unregister = deps.unregister ?? unregisterDomain;
    // Hydrate at construction (module import, before any render): first-touch
    // hydration inside a getter would mutate state while a $derived reads it.
    if (browser) this.#ensureHydrated();
  }

  // --- reads ---

  /** Open threads (not ended, touched within 30 days), newest first. */
  get open(): readonly ReadingThread[] {
    const now = this.#now();
    return this.#env.threads.filter((t) => isOpen(t, now));
  }

  /** Newest open thread — what "Continue reading" resumes. */
  get latest(): ReadingThread | null {
    return this.open[0] ?? null;
  }

  /** This tab's thread (a draft until something is read). */
  get current(): ReadingThread | null {
    const id = this.#tabId;
    if (id === null) return null;
    const draft = this.#draft;
    if (draft !== null && draft.id === id) return draft;
    return this.#env.threads.find((t) => t.id === id) ?? null;
  }

  /** Thread id the reader may offer "Start over" for, else null. */
  get offer(): string | null {
    return this.#offer;
  }

  get archive(): ArchiveView {
    return this.#archive;
  }

  /** Rows the server refused, kept here and not resent until edited. */
  get rejectedCount(): number {
    return this.#rejected;
  }

  /** Rows waiting for room under the account's quota. */
  get cappedCount(): number {
    return this.#capped;
  }

  /** This build is below the server's minimum protocol. Reading keeps working. */
  get updateRequired(): boolean {
    return this.#updateRequired;
  }

  /** The browser refused a durable write. Never report those rows as synced. */
  get storageFailed(): boolean {
    return this.#storageFailed;
  }

  /** Furthest ayah reached in a surah (account-level, never regresses). */
  furthestIn(surah: number): number | null {
    return this.#env.progress[surah]?.furthestAyah ?? null;
  }

  // --- reader integration ---

  /**
   * Resolve the tab's thread for a reader route:
   * explicit resume → that thread; untagged continuation → keep; exact-ayah,
   * card and snippet entries → new; whole-scope entries → resume the open
   * thread already in that scope (with a "Start over" offer), else new.
   */
  enter(target: ReaderTarget): EnterResult {
    this.syncOwner();
    const intent = consumeEntry();
    this.#offer = null;
    if (intent?.origin === "resume" && intent.threadId !== undefined) {
      const thread = this.#find(intent.threadId);
      if (thread !== undefined) {
        const reopened = thread.endedAt === null ? thread : this.#reopen(thread);
        this.#setTab(reopened.id, target.scope);
        return { thread: reopened, resumed: false };
      }
    }
    const tab = this.current;
    const tabAlive = tab !== null && (tab === this.#draft || isOpen(tab, this.#now()));
    if (intent === null && tab !== null && tabAlive) {
      if (this.#tabScope === target.scope || cameFromReader()) {
        this.#setTab(tab.id, target.scope);
        return { thread: tab, resumed: false };
      }
    }
    const origin = originOf(intent);
    const scope = intent?.scope ?? target.scope;
    const exact = target.exact || intent?.exact === true;
    if (!exact && origin !== "card" && origin !== "snippet") {
      const existing = this.#openInScope(scope, target.surah);
      if (existing !== undefined) {
        this.#setTab(existing.id, target.scope);
        const atStart = existing.surah === target.surah && existing.ayah === target.ayah;
        if (!atStart) this.#offer = existing.id;
        return { thread: existing, resumed: !atStart };
      }
    }
    const start = exact ? null : rangeStart(scope);
    const draft = this.#newThread({
      origin,
      originRef: intent?.ref ?? null,
      scope,
      boundEnd: intent?.boundEnd ?? target.boundEnd ?? boundFromScope(scope),
      surah: start?.surah ?? target.surah,
      ayah: start?.ayah ?? target.ayah,
      sourceId: target.sourceId,
    });
    this.#draft = draft;
    this.#setTab(draft.id, target.scope);
    return { thread: draft, resumed: false };
  }

  /** The reader's position moved (topmost verse on screen). */
  cursor(surah: number, ayah: number, sourceId: string | null): void {
    const thread = this.current ?? this.#implicitThread(surah, ayah, sourceId);
    const moved = thread.surah !== surah || thread.ayah !== ayah || thread.sourceId !== sourceId;
    // An unmoved cursor is no evidence of reading: a draft stays a draft
    // (the reader reports its first verse on open), a row stays untouched.
    if (!moved) return;
    this.#write({
      ...thread,
      surah,
      ayah,
      sourceId,
      localPage: null,
      ratio: null,
      device: this.#device,
      updatedAt: nowIso(),
    });
    this.#bumpProgress(surah, ayah);
  }

  /** Fine position inside the cursor's local page (precise handoff restore). */
  anchor(localPage: number, ratio: number): void {
    const thread = this.current;
    if (thread === null || thread === this.#draft) return;
    const rounded = roundRatio(ratio);
    if (thread.localPage === localPage && thread.ratio === rounded) return;
    this.#write({ ...thread, localPage, ratio: rounded, device: this.#device, updatedAt: nowIso() });
  }

  /** A verse was actually read (read tracker): bounded threads complete at their end. */
  noteRead(surah: number, ayah: number): void {
    let thread = this.current;
    if (thread === null || thread.endedAt !== null) return;
    // A verse read past the cursor moves it there (a page read in place, no
    // scrolling); the first read of a draft makes the thread real.
    const ahead = compareVerse({ surah, ayah }, thread) > 0;
    if (thread === this.#draft || ahead) {
      thread = {
        ...thread,
        surah: ahead ? surah : thread.surah,
        ayah: ahead ? ayah : thread.ayah,
        localPage: ahead ? null : thread.localPage,
        ratio: ahead ? null : thread.ratio,
        device: this.#device,
        updatedAt: nowIso(),
      };
      this.#write(thread);
    }
    this.#bumpProgress(surah, ayah);
    if (thread.boundEnd === null) return;
    const end = parseVerseKey(thread.boundEnd);
    if (end === null || compareVerse({ surah, ayah }, end) < 0) return;
    this.#end(thread, "completed");
  }

  /** "Start over": retire the resumed thread and begin a fresh one at the scope start. */
  startOver(): ReadingThread | null {
    const thread = this.current;
    if (thread === null) return null;
    this.#offer = null;
    if (thread !== this.#draft) this.#end(thread, "restarted");
    const start = rangeStart(thread.scope);
    const draft = this.#newThread({
      origin: thread.origin,
      originRef: thread.originRef,
      scope: thread.scope,
      boundEnd: thread.boundEnd,
      surah: start?.surah ?? thread.surah,
      ayah: start?.ayah ?? 1,
      sourceId: thread.sourceId,
    });
    this.#draft = draft;
    this.#setTab(draft.id, this.#tabScope ?? thread.scope);
    return draft;
  }

  dismissOffer(): void {
    this.#offer = null;
  }

  /** Tag the next navigation as an explicit resume of `id`. */
  markResume(id: string): void {
    markEntry({ origin: "resume", threadId: id });
  }

  /** Remove from Continue (kept in history). */
  dismiss(id: string): void {
    const thread = this.#find(id);
    if (thread === undefined || thread.endedAt !== null) return;
    this.#end(thread, "dismissed");
  }

  /** Delete from history everywhere. Signed in, the row stays as a tombstone until acknowledged. */
  remove(id: string): void {
    const thread = this.#find(id);
    this.#archive = {
      ...this.#archive,
      items: this.#archive.items.filter((t) => t.id !== id),
    };
    if (this.#tabId === id) this.#clearTab();
    if (this.#accountOwnerId() === null) {
      this.#env = { ...this.#env, threads: this.#env.threads.filter((t) => t.id !== id) };
      this.#persist.schedule();
      return;
    }
    if (thread === undefined) return;
    this.#write({ ...thread, state: "deleted", updatedAt: nowIso() });
  }

  /**
   * Clear-all history: tombstone every ended thread, server pages included. Returns
   * the threads as they were so Undo can restore them. Null when the whole history
   * could not be read, so nothing is cleared and nothing is half-cleared.
   */
  async clearHistory(): Promise<HistoryClear | null> {
    this.syncOwner();
    await this.#loadWholeArchive();
    if (!this.#archive.loaded || this.#archive.next !== null) return null;
    const now = this.#now();
    const ended = new Map<string, ReadingThread>();
    // Local rows first: they carry this device's newest copy of a thread.
    for (const thread of [...this.#env.threads, ...this.#archive.items]) {
      if (thread.state === "deleted" || isOpen(thread, now) || ended.has(thread.id)) continue;
      ended.set(thread.id, thread);
    }
    const threads = [...ended.values()];
    for (const thread of threads) {
      if (this.#tabId === thread.id) this.#clearTab();
      this.#write({ ...thread, state: "deleted", updatedAt: nowIso() });
    }
    this.#archive = { items: [], next: null, loading: false, loaded: true };
    return { owner: this.#owner, threads };
  }

  /** Undo for `clearHistory`: write each thread back with a fresh lifecycle stamp. */
  restoreHistory(clear: HistoryClear): void {
    this.syncOwner();
    // Another account is signed in now: its history must not gain these rows.
    if (clear.owner !== this.#owner) return;
    for (const thread of clear.threads) {
      const tombstone = this.#env.threads.find((t) => t.id === thread.id);
      this.#write({ ...thread, updatedAt: restoreStamp(tombstone) });
    }
    // The snapshot was the whole history (clearHistory needs it complete), so the
    // view goes back to exactly that, without a refetch that could race the sync.
    this.#archive = {
      items: [...clear.threads].sort(byNewest),
      next: null,
      loading: false,
      loaded: true,
    };
  }

  /** Load every archive page, stopping early if the server stops advancing. */
  async #loadWholeArchive(): Promise<void> {
    await this.loadArchive(true);
    for (let page = 0; page < MAX_ARCHIVE_PAGES; page += 1) {
      const cursor = this.#archive.next;
      if (!this.#archive.loaded || cursor === null) return;
      await this.loadArchive();
      if (this.#archive.next === cursor) return;
    }
  }

  /** History: ended + stale threads, newest first. Account pages come from the server. */
  async loadArchive(reset = false): Promise<void> {
    this.#ensureHydrated();
    const prev = this.#archive;
    if (prev.loading || (!reset && prev.loaded && prev.next === null)) return;
    const ownerId = this.#accountOwnerId();
    if (ownerId === null) {
      const now = this.#now();
      const items = this.#env.threads.filter((t) => t.state !== "deleted" && !isOpen(t, now));
      this.#archive = { items, next: null, loading: false, loaded: true };
      return;
    }
    const cursor = reset ? null : prev.next;
    this.#archive = { ...prev, items: reset ? [] : prev.items, loading: true };
    const query = new URLSearchParams({ limit: String(ARCHIVE_PAGE) });
    if (cursor !== null) query.set("before", cursor);
    try {
      const res = await this.#auth.get<unknown>(`/reading-session/v1/archive?${query}`);
      const page = res.ok ? decodeArchivePage(res.data) : null;
      if (this.#accountOwnerId() !== ownerId) return;
      if (page === null) {
        this.#archive = { ...this.#archive, loading: false };
        return;
      }
      const seen = new Set(this.#archive.items.map((t) => t.id));
      this.#archive = {
        items: [...this.#archive.items, ...page.sessions.filter((t) => !seen.has(t.id))],
        next: page.next,
        loading: false,
        loaded: true,
      };
    } catch {
      this.#archive = { ...this.#archive, loading: false };
    }
  }

  // --- auth + sync ---

  /** Login / logout / expiry edge: swap owners; on sign-in fold device threads into the account. */
  onAuthTransition(): void {
    this.syncOwner();
    const ownerId = this.#accountOwnerId();
    if (ownerId !== null) this.#registerFor(ownerId);
  }

  /** Re-resolve the owner (cheap; every public write path calls it). */
  syncOwner(): void {
    this.#ensureHydrated();
    const next = this.#resolveOwner();
    if (next === this.#owner) return;
    this.#flushPending();
    const absorbing = this.#owner === LOCAL_OWNER && next !== LOCAL_OWNER;
    const local = absorbing ? this.#env : null;
    this.#owner = next;
    this.#unregisterCurrent();
    this.#cappedIds.clear();
    this.#fullPullSeen = null;
    this.#archive = { items: [], next: null, loading: false, loaded: false };
    this.#load(next);
    if (local !== null) this.#absorb(local);
  }

  /** Rows the next round sends. A capped row waits behind the others. */
  dirtyRows(): readonly SessionWireRow[] {
    const dirty = this.#env.threads.filter((t) => t.dirty && !t.rejected && t.id.length === 36);
    const ready = dirty.filter((t) => !this.#cappedIds.has(t.id));
    const waiting = dirty.filter((t) => this.#cappedIds.has(t.id));
    const threads = [...ready, ...waiting].map((t) => wireOf(t));
    const progress = Object.entries(this.#env.progress)
      .filter(([, row]) => row.dirty === true)
      .map(([surah, row]) => ({
        kind: "progress.upsert" as const,
        surah: Number(surah),
        furthestAyah: row.furthestAyah,
        updatedAt: row.updatedAt,
      }));
    return [...threads, ...progress].slice(0, SYNC_PAGE);
  }

  /**
   * Apply one round durably. `dirty` clears only for a row whose current stamps are
   * the ones sent, so an edit made while the request flew stays queued. A row
   * missing from a response means nothing: only a tombstone deletes.
   */
  applyRound(round: SessionRound, sent: readonly SessionWireRow[]) {
    const clock = this.#clock;
    clock?.onServerTime(round.serverTime);
    // Everything below is synchronous, so another tab's writes join once, here.
    this.#reloadFromStorage(this.#owner);

    const epochChanged = this.#env.epoch !== null && round.syncEpoch !== this.#env.epoch;
    const aheadOfServer = this.#env.since > round.latestRev;
    const belowFloor = this.#env.since > 0 && this.#env.since < round.tombstoneFloor;
    const restored = epochChanged || aheadOfServer;

    const byId = new Map(this.#env.threads.map((t) => [t.id, t]));
    const progressNow = new Map(
      Object.entries(this.#env.progress).map(([surah, row]) => [Number(surah), row] as const),
    );
    const sentById = new Map(
      sent.flatMap((row) => (row.kind === "session.upsert" ? [[row.id, row] as const] : [])),
    );

    for (const result of round.results) {
      this.#applyResult(result, sentById.get(result.id), byId);
      this.#applyProgressAck(result, sent, progressNow);
    }
    for (const row of round.sessions) {
      this.#observeRow(row);
      const mine = byId.get(row.id);
      byId.set(row.id, mine === undefined ? row : mergeRow(mine, row));
      this.#fullPullSeen?.add(row.id);
    }
    for (const tombstone of round.tombstones) this.#applyTombstone(tombstone, byId);
    for (const row of round.progress) {
      const prev = progressNow.get(row.surah);
      if (prev === undefined || row.furthestAyah >= prev.furthestAyah) {
        progressNow.set(row.surah, { furthestAyah: row.furthestAyah, updatedAt: row.updatedAt });
      }
    }

    if (belowFloor) this.#fullPullSeen = new Set();
    if (restored) {
      // Rows and `rev` went backwards: every row this device holds is pushed
      // again, which heals writes the restore lost.
      for (const [id, thread] of byId) byId.set(id, { ...thread, dirty: true });
    }

    const reset = restored || belowFloor;
    const fullPull = this.#env.since === 0;
    let after = -1;
    let since = reset ? 0 : round.rev;
    if (!reset && round.more && fullPull) {
      since = 0;
      after = round.rev;
    }
    if (!reset && !round.more && this.#fullPullSeen !== null) {
      // Past the floor, the server no longer holds tombstones for what is gone.
      for (const [id, thread] of byId) {
        if (!thread.dirty && !this.#fullPullSeen.has(id)) byId.set(id, this.#goneLocally(thread));
      }
      this.#fullPullSeen = null;
    }

    this.#env = {
      ...this.#env,
      threads: mergeThreads([...byId.values()], [], this.#now()),
      progress: Object.fromEntries(progressNow),
      since,
      after,
      epoch: round.syncEpoch ?? this.#env.epoch,
    };
    // `since` is saved with the rows it covers, so an interrupted pull resumes.
    this.#store();

    const sentAll = sent.length >= SYNC_PAGE;
    const progressed = round.results.some((r) => r.status === "applied" || r.status === "superseded");
    const dirtyLeft = this.dirtyRows().length > 0;
    return { more: reset || round.more || (sentAll && progressed && dirtyLeft) };
  }

  #observeRow(row: ThreadStamps): void {
    this.#clock?.observe(row.identityStamp);
    this.#clock?.observe(row.cursorStamp);
    this.#clock?.observe(row.lifecycleStamp);
  }

  #applyResult(
    result: RowResult,
    sent: SessionWireRow | undefined,
    byId: Map<string, ReadingThread>,
  ): void {
    const mine = byId.get(result.id);
    if (mine === undefined || sent === undefined || sent.kind !== "session.upsert") return;
    const unchanged =
      mine.identityStamp === (sent.identityStamp ?? null) &&
      mine.cursorStamp === (sent.cursorStamp ?? null) &&
      mine.lifecycleStamp === (sent.lifecycleStamp ?? null);
    this.#cappedIds.delete(result.id);
    if (result.status === "capped") {
      this.#cappedIds.add(result.id);
      this.#capped = this.#cappedIds.size;
      return;
    }
    this.#capped = this.#cappedIds.size;
    if (result.status === "rejected") {
      if (unchanged) byId.set(result.id, { ...mine, rejected: true });
      this.#rejected = [...byId.values()].filter((t) => t.rejected).length;
      return;
    }
    if (!unchanged) return;
    if (result.stamps === null) {
      // Older than the tombstone floor: it may have been deleted long ago. It
      // cannot come back, so the device lets it go.
      byId.set(result.id, this.#goneLocally(mine));
      return;
    }
    // A stamp the server rewrote is below ours: adopt it, or this device would
    // keep a row no other replica holds. One the server beat arrives with the row.
    const adopt = (stamp: string | null | undefined, local: string | null): string | null =>
      stamp !== null && stamp !== undefined && local !== null && stamp < local ? stamp : local;
    const adopted = {
      ...mine,
      identityStamp: adopt(result.stamps.identity, mine.identityStamp),
      cursorStamp: adopt(result.stamps.cursor, mine.cursorStamp),
      lifecycleStamp: adopt(result.stamps.lifecycle, mine.lifecycleStamp),
      dirty: false,
      rejected: false,
    };
    byId.set(result.id, adopted);
  }

  #applyProgressAck(
    result: RowResult,
    sent: readonly SessionWireRow[],
    progress: Map<number, SurahProgress>,
  ): void {
    if (result.status === "rejected" || !result.id.startsWith("progress:")) return;
    const surah = Number(result.id.slice("progress:".length));
    const row = sent.find((m) => m.kind === "progress.upsert" && m.surah === surah);
    const current = progress.get(surah);
    if (row?.kind !== "progress.upsert" || current === undefined) return;
    if (current.furthestAyah === row.furthestAyah) {
      progress.set(surah, { furthestAyah: current.furthestAyah, updatedAt: current.updatedAt });
    }
  }

  /** Delete only on a tombstone. A local undo with a newer stamp keeps its row. */
  #applyTombstone(tombstone: ThreadTombstone, byId: Map<string, ReadingThread>): void {
    this.#observeRow(tombstone);
    const mine = byId.get(tombstone.id);
    if (mine === undefined) return;
    const localWins =
      mine.lifecycleStamp !== null &&
      compareStamps(mine.lifecycleStamp, tombstone.lifecycleStamp) > 0;
    if (localWins) return;
    if (mine.dirty) {
      byId.set(tombstone.id, {
        ...mine,
        state: "deleted",
        lifecycleStamp: tombstone.lifecycleStamp,
      });
      return;
    }
    byId.set(tombstone.id, {
      ...mine,
      state: "deleted",
      lifecycleStamp: tombstone.lifecycleStamp,
    });
    if (this.#tabId === tombstone.id) this.#clearTab();
  }

  /**
   * Tombstones stay as clean local rows until the cap trims them: another tab
   * merging its copy can then never bring a deleted thread back. This one was never
   * on the server, so its stamp is purely local.
   */
  #goneLocally(thread: ReadingThread): ReadingThread {
    return {
      ...thread,
      state: "deleted",
      lifecycleStamp: this.#clock?.stamp() ?? thread.lifecycleStamp,
      dirty: false,
    };
  }

  // --- internals ---

  #find(id: string): ReadingThread | undefined {
    if (this.#draft?.id === id) return this.#draft;
    return (
      this.#env.threads.find((t) => t.id === id) ?? this.#archive.items.find((t) => t.id === id)
    );
  }

  #openInScope(scope: string, surah: number): ReadingThread | undefined {
    const surahScope = scope.startsWith("surah:");
    return this.open.find(
      (t) => t.scope === scope || (surahScope && t.scope.startsWith("surah:") && t.surah === surah),
    );
  }

  #newThread(fields: {
    origin: SessionOrigin;
    originRef: string | null;
    scope: string;
    boundEnd: string | null;
    surah: number;
    ayah: number;
    sourceId: string | null;
  }): ReadingThread {
    const now = nowIso();
    return {
      id: newEntityId(),
      ...fields,
      localPage: null,
      ratio: null,
      device: this.#device,
      startedAt: now,
      updatedAt: now,
      endedAt: null,
      endReason: null,
      state: "open",
      deviceId: null,
      identityStamp: null,
      cursorStamp: null,
      lifecycleStamp: null,
      dirty: false,
      rejected: false,
      extras: {},
    };
  }

  #implicitThread(surah: number, ayah: number, sourceId: string | null): ReadingThread {
    const draft = this.#newThread({
      origin: "link",
      originRef: null,
      scope: `surah:${surah}`,
      boundEnd: null,
      surah,
      ayah,
      sourceId,
    });
    this.#draft = draft;
    this.#setTab(draft.id, draft.scope);
    return draft;
  }

  #reopen(thread: ReadingThread): ReadingThread {
    const reopened: ReadingThread = {
      ...thread,
      state: "open",
      endedAt: null,
      endReason: null,
      updatedAt: nowIso(),
    };
    this.#write(reopened);
    return reopened;
  }

  #end(thread: ReadingThread, reason: EndReason): void {
    const now = nowIso();
    this.#write({ ...thread, state: "ended", endedAt: now, endReason: reason, updatedAt: now });
    if (this.#offer === thread.id) this.#offer = null;
  }

  /**
   * Commit a thread row: drafts materialize here, the first time anything is read.
   * Only a register whose fields changed gets a new stamp, so a cursor move can
   * never reopen a thread and a lifecycle write can never move a cursor.
   */
  #write(thread: ReadingThread): void {
    this.syncOwner();
    if (this.#draft?.id === thread.id) this.#draft = null;
    const prev = this.#env.threads.find((t) => t.id === thread.id);
    const lifecycleMoved =
      prev === undefined ||
      prev.state !== thread.state ||
      prev.endedAt !== thread.endedAt ||
      prev.endReason !== thread.endReason;
    const row = this.#stamped(thread, prev, lifecycleMoved);
    this.#env = {
      ...this.#env,
      threads: mergeThreads(
        this.#env.threads.filter((t) => t.id !== row.id),
        [row],
        this.#now(),
      ),
    };
    // Lifecycle writes persist at once. Cursor moves may debounce, so a hard kill
    // can lose only the last moments of scrolling.
    if (lifecycleMoved) this.#writeNow();
    else this.#persist.schedule();
    if (this.#accountOwnerId() !== null) this.#schedulePush.schedule();
  }

  #stamped(
    thread: ReadingThread,
    prev: ReadingThread | undefined,
    lifecycleMoved: boolean,
  ): ReadingThread {
    const clock = this.#clock;
    if (clock === null) return thread;
    const cursorMoved =
      prev === undefined ||
      prev.surah !== thread.surah ||
      prev.ayah !== thread.ayah ||
      prev.localPage !== thread.localPage ||
      prev.ratio !== thread.ratio ||
      prev.sourceId !== thread.sourceId ||
      prev.device !== thread.device;
    return {
      ...thread,
      deviceId: thread.deviceId ?? this.#deviceId,
      identityStamp: thread.identityStamp ?? clock.stamp(),
      cursorStamp: cursorMoved || thread.cursorStamp === null ? clock.stamp() : thread.cursorStamp,
      lifecycleStamp:
        lifecycleMoved || thread.lifecycleStamp === null ? clock.stamp() : thread.lifecycleStamp,
      dirty: true,
      rejected: false,
    };
  }

  #now(): number {
    return this.#clock?.now() ?? Date.now();
  }

  #bumpProgress(surah: number, ayah: number): void {
    const prev = this.#env.progress[surah];
    if (prev !== undefined && prev.furthestAyah >= ayah) return;
    const account = this.#accountOwnerId() !== null;
    const row: SurahProgress = account
      ? { furthestAyah: ayah, updatedAt: nowIso(), dirty: true }
      : { furthestAyah: ayah, updatedAt: nowIso() };
    this.#env = { ...this.#env, progress: { ...this.#env.progress, [surah]: row } };
    this.#persist.schedule();
    if (account) this.#schedulePush.schedule();
  }

  #setTab(id: string, scope: string): void {
    this.#tabId = id;
    this.#tabScope = scope;
    writeRaw("session", TAB_KEY, JSON.stringify({ id, scope }));
  }

  #clearTab(): void {
    this.#tabId = null;
    this.#tabScope = null;
    this.#draft = null;
    writeRaw("session", TAB_KEY, "");
  }

  #loadTab(): void {
    const obj = parseTabRecord(readRaw("session", TAB_KEY));
    const id = asString(obj?.id);
    const scope = asString(obj?.scope);
    if (id === undefined || scope === undefined) return;
    const thread = isUuidV7(id) ? undefined : this.#legacyTabTarget(id);
    this.#tabId = thread ?? id;
    this.#tabScope = scope;
  }

  /** A tab record written before re-keying: follow its thread to the new id, keyed the same way. */
  #legacyTabTarget(id: string): string | undefined {
    return this.#env.threads.find((t) => t.id === uuidV7FromLegacy(id, Date.parse(t.startedAt)))
      ?.id;
  }

  #resolveOwner(): string {
    if (this.#state.authenticated && this.#state.user !== null) return String(this.#state.user.id);
    return LOCAL_OWNER;
  }

  #accountOwnerId(): number | null {
    if (this.#owner === LOCAL_OWNER) return null;
    const id = Number(this.#owner);
    return Number.isSafeInteger(id) && id > 0 ? id : null;
  }

  #ensureHydrated(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#owner = this.#resolveOwner();
    this.#load(this.#owner);
    this.#loadTab();
    this.#pruneIdleAccounts();
    onPageHide(() => this.#flushPending());
    // Another tab of this owner wrote: fold its threads into ours (multi-tab).
    window.addEventListener("storage", (event) => {
      if (event.key === keyFor(this.#owner)) this.#reloadFromStorage(this.#owner);
    });
  }

  #readStored(owner: string): SessionsEnvelope | undefined {
    const raw = readJSON(keyFor(owner));
    if (isNewerStore(raw)) {
      this.#futureStore = true;
      return undefined;
    }
    const env = decodeEnvelope(raw);
    if (env === undefined) return undefined;
    return { ...env, threads: this.#rekeyLegacy(env.threads) };
  }

  /**
   * The server refuses non-v7 session ids: a stored thread with an older id gets a
   * v7 built from its `startedAt` and syncs again as a new row.
   */
  #rekeyLegacy(threads: readonly ReadingThread[]): readonly ReadingThread[] {
    if (threads.every((t) => isUuidV7(t.id))) return threads;
    const rekeyed = threads.map((thread) => {
      if (isUuidV7(thread.id)) return thread;
      const id = uuidV7FromLegacy(thread.id, Date.parse(thread.startedAt));
      if (this.#tabId === thread.id) this.#tabId = id;
      return { ...thread, id, dirty: true, rejected: false };
    });
    this.#persist.schedule();
    return rekeyed;
  }

  /** Rows from before stamps get them now, from their own time. */
  #stampLoaded(threads: readonly ReadingThread[]): readonly ReadingThread[] {
    const clock = this.#clock;
    if (clock === null) return threads;
    return threads.map((thread) => {
      if (thread.cursorStamp !== null) return thread;
      const at = Date.parse(thread.updatedAt);
      const stamp = clock.stampAt(Number.isFinite(at) ? at : 0);
      return {
        ...thread,
        deviceId: thread.deviceId ?? this.#deviceId,
        identityStamp: stamp,
        cursorStamp: stamp,
        lifecycleStamp: stamp,
        dirty: true,
      };
    });
  }

  #load(owner: string): void {
    this.#futureStore = false;
    const accountId = owner === LOCAL_OWNER ? null : owner;
    this.#deviceId = accountId === null ? null : deviceIdFor(accountId);
    this.#clock =
      accountId === null || this.#deviceId === null
        ? null
        : createClock(this.#deviceId, localClockStorage(accountId));
    const stored = this.#readStored(owner);
    const env = stored ?? emptyEnvelope();
    this.#env = { ...env, threads: [...this.#stampLoaded(env.threads)] };
    for (const thread of env.threads) this.#observeRow(thread);
    this.#rejected = env.threads.filter((t) => t.rejected).length;
    this.#capped = 0;
    if (accountId !== null) {
      this.#env = { ...this.#env, lastSeenAt: Date.now() };
      this.#persist.schedule();
    }
  }

  #reloadFromStorage(owner: string): void {
    if (owner !== this.#owner) return;
    const stored = this.#readStored(owner);
    if (stored === undefined) return;
    this.#env = {
      ...this.#env,
      threads: mergeThreads(this.#env.threads, stored.threads, this.#now()),
      progress: mergeProgress(this.#env.progress, stored.progress),
      since: this.#env.since === 0 ? 0 : Math.max(this.#env.since, stored.since),
      epoch: this.#env.epoch ?? stored.epoch,
    };
  }

  #writeNow(): void {
    if (!browser || this.#futureStore) return;
    // Merge-on-write: another tab may have written since we loaded.
    this.#reloadFromStorage(this.#owner);
    this.#store();
  }

  #store(): void {
    if (!browser || this.#futureStore) return;
    const landed = tryWriteJSON(keyFor(this.#owner), this.#env);
    if (landed === this.#storageFailed) {
      this.#storageFailed = !landed;
      this.#engine.setStorageDegraded?.(!landed);
    }
  }

  #flushPending(): void {
    this.#persist.flush();
    this.#schedulePush.flush();
  }

  /** Sign-in: the device's anonymous threads + progress join the account, silently. */
  #absorb(local: SessionsEnvelope): void {
    if (local.threads.length === 0 && Object.keys(local.progress).length === 0) return;
    const clock = this.#clock;
    const threads = local.threads.map((thread) => {
      const at = Date.parse(thread.updatedAt);
      const stamp = clock?.stampAt(Number.isFinite(at) ? at : 0) ?? null;
      return {
        ...thread,
        deviceId: this.#deviceId,
        identityStamp: stamp,
        cursorStamp: stamp,
        lifecycleStamp: stamp,
        dirty: true,
        rejected: false,
      };
    });
    const progress = Object.fromEntries(
      Object.entries(local.progress).map(([surah, row]) => [surah, { ...row, dirty: true }]),
    );
    this.#env = {
      ...this.#env,
      threads: mergeThreads(this.#env.threads, threads, this.#now()),
      progress: mergeProgress(this.#env.progress, progress),
    };
    removeJSON(keyFor(LOCAL_OWNER));
    this.#persist.schedule();
    this.#schedulePush.schedule();
  }

  #push(): void {
    const ownerId = this.#accountOwnerId();
    if (ownerId === null || this.#registered !== sessionsDomainFor(ownerId)) return;
    this.#persist.flush();
    void this.#engine.flush(sessionsDomainFor(ownerId)).catch(() => undefined);
  }

  /** Other accounts' cached rows go after 180 days away. Dirty rows wait for sign-in. */
  #pruneIdleAccounts(): void {
    const prefix = keyFor("");
    const now = Date.now();
    try {
      const keys = Object.keys(localStorage).filter(
        (key) => key.startsWith(prefix) && key !== keyFor(this.#owner) && key !== keyFor(LOCAL_OWNER),
      );
      for (const key of keys) {
        const env = decodeEnvelope(readJSON(key));
        if (env === undefined || env.lastSeenAt === 0) continue;
        if (now - env.lastSeenAt < IDLE_ACCOUNT_MS) continue;
        tryWriteJSON(key, { ...env, threads: env.threads.filter((t) => t.dirty) });
      }
    } catch {
      return;
    }
  }

  #unregisterCurrent(): void {
    if (this.#registered === null) return;
    this.#unregister(this.#registered);
    this.#registered = null;
  }

  #registerFor(ownerId: number): void {
    const name = sessionsDomainFor(ownerId);
    if (this.#registered === name) return;
    this.#unregisterCurrent();
    if (this.#futureStore) return;
    const isCurrent = (): boolean =>
      this.#registered === name && this.#accountOwnerId() === ownerId;
    const adapter = createSessionsDomain(
      this.#auth,
      this.#state,
      ownerId,
      {
        dirtyRows: () => this.dirtyRows(),
        position: () => ({ since: this.#env.since, after: this.#env.after }),
        applyRound: (round, sent) => this.applyRound(round, sent),
        onUpgradeRequired: () => {
          this.#updateRequired = true;
        },
      },
      isCurrent,
      () => {
        if (!isCurrent()) return;
        this.#unregisterCurrent();
        this.#state.notifySessionLost?.();
      },
    );
    this.#register(adapter);
    this.#registered = name;
    this.#engine.start();
    // Anything written before registration (late auth probe, absorb) goes now.
    this.#push();
    void this.#engine.flush(name).catch(() => undefined);
  }
}

export const sessions: SessionsStore = new SessionsStore();
