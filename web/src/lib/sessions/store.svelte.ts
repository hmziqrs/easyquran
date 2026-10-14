import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import {
  asArray,
  asObject,
  asString,
  onPageHide,
  readJSON,
  readRaw,
  removeJSON,
  trailingDebounce,
  writeJSON,
  writeRaw,
} from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  type RegisteredSyncDomain,
  type SyncDomain,
  type SyncMutation,
} from "#lib/sync/index.js";

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
  SESSIONS_SCHEMA_VERSION,
  byNewest,
  compareVerse,
  decodeArchivePage,
  decodeEnvelope,
  emptyEnvelope,
  isOpen,
  parseVerseKey,
  type EndReason,
  type ReadingThread,
  type SessionMutation,
  type SessionOrigin,
  type SessionSnapshot,
  type ProgressBySurah,
  type SessionsEnvelope,
  type SurahProgress,
} from "./schema";

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SessionsEngineLike {
  enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>>;
  flush(domain?: string): Promise<void>;
  start(): () => void;
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
 * Reading threads (docs/reading-model.md §2). Per-owner durable key
 * `easyquran.sessions.{owner}` (owner = user id when signed in, `local`
 * otherwise); the tab's current thread id lives in sessionStorage so a reload
 * continues it and two tabs read two threads. A thread the reader merely
 * opened stays an in-memory draft until something is actually read — bounces
 * never litter Continue. Signed in, every thread row and progress change
 * syncs through the `sessions.user:{id}` outbox partition; on sign-in the
 * device's anonymous threads fold into the account silently.
 */

const LOCAL_OWNER = "local";
const TAB_KEY = "easyquran.session.tab";
const REMOVED_CAP = 200;
/** Cursor moves coalesce: scrolling marks many verses in seconds. */
const PUSH_DEBOUNCE_MS = 4000;
const PERSIST_DEBOUNCE_MS = 400;
const ARCHIVE_PAGE = 20;

function keyFor(owner: string): string {
  return `easyquran.sessions.${owner}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter narrows it to a displayable message string.
function errorMessage(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
}

/** Durable envelope + local removal ledger (ids deleted from history on this device). */
type StoredEnvelope = SessionsEnvelope & { readonly removed?: readonly string[] };

function newerOf(a: ReadingThread, b: ReadingThread): ReadingThread {
  return b.updatedAt > a.updatedAt ? b : a;
}

/** Union by id, newest row wins; open threads are kept before the cap trims history. */
function mergeThreads(
  a: readonly ReadingThread[],
  b: readonly ReadingThread[],
  removed: ReadonlySet<string>,
): ReadingThread[] {
  const byId = new Map<string, ReadingThread>();
  for (const row of [...a, ...b]) {
    if (removed.has(row.id)) continue;
    const prev = byId.get(row.id);
    byId.set(row.id, prev === undefined ? row : newerOf(prev, row));
  }
  const all = [...byId.values()].sort(byNewest);
  const now = Date.now();
  const open = all.filter((t) => isOpen(t, now));
  const rest = all.filter((t) => !isOpen(t, now));
  return [...open, ...rest].slice(0, LOCAL_THREAD_CAP).sort(byNewest);
}

function mergeProgress(a: ProgressBySurah, b: ProgressBySurah): ProgressBySurah {
  const out = { ...a };
  for (const [key, row] of Object.entries(b)) {
    const surah = Number(key);
    const prev = out[surah];
    if (prev === undefined || row.furthestAyah > prev.furthestAyah) out[surah] = row;
  }
  return out;
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
  #removed = new Set<string>();
  /** The tab's thread before anything was read in it — never persisted or synced. */
  #draft = $state.raw<ReadingThread | null>(null);
  #tabId = $state<string | null>(null);
  #tabScope: string | null = null;
  /** A whole-scope entry resumed this thread: offer "Start over". */
  #offer = $state<string | null>(null);
  #archive = $state.raw<ArchiveView>({ items: [], next: null, loading: false, loaded: false });
  #enqueueError = $state<string | null>(null);

  #dirtyThreads = new Set<string>();
  #dirtyProgress = new Set<number>();
  /** Enqueued-but-undrained rows: they beat any snapshot row until their mutation drains. */
  #pendingThreads = new Map<string, ReadingThread>();
  #pendingDeletes = new Set<string>();
  #registered: string | null = null;
  #device = deviceKind();

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
    const now = Date.now();
    return this.#env.threads.filter((t) => isOpen(t, now) && !this.#pendingDeletes.has(t.id));
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

  get enqueueError(): string | null {
    return this.#enqueueError;
  }

  /** Furthest ayah reached in a surah (account-level, never regresses). */
  furthestIn(surah: number): number | null {
    return this.#env.progress[surah]?.furthestAyah ?? null;
  }

  // --- reader integration ---

  /**
   * Resolve the tab's thread for a reader route (docs/reading-model.md §2.1):
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
    const tabAlive = tab !== null && (tab === this.#draft || isOpen(tab));
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
    if (thread.localPage === localPage && thread.ratio === ratio) return;
    this.#write({ ...thread, localPage, ratio, device: this.#device, updatedAt: nowIso() });
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

  /** Delete from history everywhere (tombstone on the server). */
  remove(id: string): void {
    this.#removed.add(id);
    this.#dirtyThreads.delete(id);
    this.#pendingThreads.delete(id);
    this.#env = { ...this.#env, threads: this.#env.threads.filter((t) => t.id !== id) };
    this.#archive = {
      ...this.#archive,
      items: this.#archive.items.filter((t) => t.id !== id),
    };
    if (this.#tabId === id) this.#clearTab();
    this.#persist.schedule();
    if (this.#accountOwnerId() !== null) {
      this.#pendingDeletes.add(id);
      this.#enqueue({ kind: "session.delete", id, updatedAt: nowIso() });
    }
  }

  /** History: ended + stale threads, newest first. Account pages come from the server. */
  async loadArchive(reset = false): Promise<void> {
    this.#ensureHydrated();
    const prev = this.#archive;
    if (prev.loading || (!reset && prev.loaded && prev.next === null)) return;
    const ownerId = this.#accountOwnerId();
    if (ownerId === null) {
      const now = Date.now();
      const items = this.#env.threads.filter((t) => !isOpen(t, now));
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
    this.#dirtyThreads.clear();
    this.#dirtyProgress.clear();
    this.#pendingThreads.clear();
    this.#pendingDeletes.clear();
    this.#archive = { items: [], next: null, loading: false, loaded: false };
    this.#load(next);
    if (local !== null) this.#absorb(local);
  }

  applyServer(snapshot: SessionSnapshot, drained?: readonly SyncMutation<SessionMutation>[]): void {
    for (const mutation of drained ?? []) {
      const payload = mutation.payload;
      if (payload.kind === "session.upsert") {
        const pending = this.#pendingThreads.get(payload.id);
        if (pending !== undefined && pending.updatedAt === payload.updatedAt) {
          this.#pendingThreads.delete(payload.id);
        }
      } else if (payload.kind === "session.delete") {
        this.#pendingDeletes.delete(payload.id);
      }
    }
    if (this.#accountOwnerId() === null) return;
    const local = new Map(this.#env.threads.map((t) => [t.id, t]));
    const serverIds = new Set<string>();
    for (const row of snapshot.sessions) {
      serverIds.add(row.id);
      if (this.#pendingDeletes.has(row.id) || this.#removed.has(row.id)) continue;
      if (this.#pendingThreads.has(row.id) || this.#dirtyThreads.has(row.id)) continue;
      const mine = local.get(row.id);
      local.set(row.id, mine === undefined ? row : newerOf(mine, row));
    }
    // The snapshot is the server's newest open threads. A local open thread
    // missing from it ended or was deleted elsewhere — unless it is older
    // than the snapshot's tail (merely cut by the limit) or still ours.
    const tail = snapshot.sessions.at(-1);
    const full = snapshot.sessions.length >= 20;
    const now = Date.now();
    for (const [id, thread] of local) {
      if (serverIds.has(id) || !isOpen(thread, now) || id === this.#tabId) continue;
      if (this.#pendingThreads.has(id) || this.#dirtyThreads.has(id)) continue;
      if (full && tail !== undefined && thread.updatedAt < tail.updatedAt) continue;
      local.delete(id);
    }
    const progress: Record<number, SurahProgress> = {};
    for (const row of snapshot.progress) {
      progress[row.surah] = { furthestAyah: row.furthestAyah, updatedAt: row.updatedAt };
    }
    this.#env = {
      v: SESSIONS_SCHEMA_VERSION,
      threads: mergeThreads([...local.values()], [], this.#removed),
      progress: mergeProgress(this.#env.progress, progress),
    };
    this.#persist.schedule();
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
    };
  }

  /** A position write with no resolved thread (a reader that never called enter). */
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
    const reopened = { ...thread, endedAt: null, endReason: null, updatedAt: nowIso() };
    this.#write(reopened);
    return reopened;
  }

  #end(thread: ReadingThread, reason: EndReason): void {
    const now = nowIso();
    this.#write({ ...thread, endedAt: now, endReason: reason, updatedAt: now });
    if (this.#offer === thread.id) this.#offer = null;
  }

  /** Commit a thread row: drafts materialize here, the first time anything is read. */
  #write(thread: ReadingThread): void {
    this.syncOwner();
    if (this.#draft?.id === thread.id) this.#draft = null;
    this.#removed.delete(thread.id);
    this.#env = {
      ...this.#env,
      threads: mergeThreads(
        this.#env.threads.filter((t) => t.id !== thread.id),
        [thread],
        this.#removed,
      ),
    };
    this.#dirtyThreads.add(thread.id);
    this.#persist.schedule();
    if (this.#accountOwnerId() !== null) this.#schedulePush.schedule();
  }

  #bumpProgress(surah: number, ayah: number): void {
    const prev = this.#env.progress[surah];
    if (prev !== undefined && prev.furthestAyah >= ayah) return;
    this.#env = {
      ...this.#env,
      progress: { ...this.#env.progress, [surah]: { furthestAyah: ayah, updatedAt: nowIso() } },
    };
    this.#dirtyProgress.add(surah);
    this.#persist.schedule();
    if (this.#accountOwnerId() !== null) this.#schedulePush.schedule();
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
    this.#tabId = id;
    this.#tabScope = scope;
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
    onPageHide(() => this.#flushPending());
    // Another tab of this owner wrote: fold its threads into ours (multi-tab).
    window.addEventListener("storage", (event) => {
      if (event.key === keyFor(this.#owner)) this.#reloadFromStorage(this.#owner);
    });
  }

  #readStored(owner: string): StoredEnvelope | undefined {
    const raw = readJSON(keyFor(owner));
    const env = decodeEnvelope(raw);
    if (env === undefined) return undefined;
    const removed = asArray(asObject(raw)?.removed, (id) => asString(id));
    return { ...env, removed };
  }

  #load(owner: string): void {
    const stored = this.#readStored(owner);
    this.#removed = new Set(stored?.removed ?? []);
    this.#env = stored ?? emptyEnvelope();
  }

  #reloadFromStorage(owner: string): void {
    if (owner !== this.#owner) return;
    const stored = this.#readStored(owner);
    if (stored === undefined) return;
    for (const id of stored.removed ?? []) this.#removed.add(id);
    this.#env = {
      v: SESSIONS_SCHEMA_VERSION,
      threads: mergeThreads(this.#env.threads, stored.threads, this.#removed),
      progress: mergeProgress(this.#env.progress, stored.progress),
    };
  }

  #writeNow(): void {
    if (!browser) return;
    // Merge-on-write: another tab may have written since we loaded.
    this.#reloadFromStorage(this.#owner);
    const removed = [...this.#removed].slice(-REMOVED_CAP);
    writeJSON(keyFor(this.#owner), { ...this.#env, removed });
  }

  #flushPending(): void {
    this.#persist.flush();
    this.#schedulePush.flush();
  }

  /** Sign-in: the device's anonymous threads + progress join the account, silently. */
  #absorb(local: SessionsEnvelope): void {
    if (local.threads.length === 0 && Object.keys(local.progress).length === 0) return;
    this.#env = {
      v: SESSIONS_SCHEMA_VERSION,
      threads: mergeThreads(this.#env.threads, local.threads, this.#removed),
      progress: mergeProgress(this.#env.progress, local.progress),
    };
    for (const thread of local.threads) this.#dirtyThreads.add(thread.id);
    for (const surah of Object.keys(local.progress)) this.#dirtyProgress.add(Number(surah));
    removeJSON(keyFor(LOCAL_OWNER));
    this.#persist.schedule();
    this.#schedulePush.schedule();
  }

  #push(): void {
    const ownerId = this.#accountOwnerId();
    if (ownerId === null || this.#registered !== sessionsDomainFor(ownerId)) return;
    for (const id of this.#dirtyThreads) {
      const thread = this.#env.threads.find((t) => t.id === id);
      if (thread === undefined) continue;
      this.#pendingThreads.set(id, thread);
      this.#enqueue({ kind: "session.upsert", ...thread });
    }
    this.#dirtyThreads.clear();
    for (const surah of this.#dirtyProgress) {
      const row = this.#env.progress[surah];
      if (row === undefined) continue;
      this.#enqueue({ kind: "progress.upsert", surah, ...row });
    }
    this.#dirtyProgress.clear();
  }

  #enqueue(payload: SessionMutation): void {
    const ownerId = this.#accountOwnerId();
    if (ownerId === null) return;
    void this.#engine.enqueue(sessionsDomainFor(ownerId), payload).then(
      () => {
        this.#enqueueError = null;
      },
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); errorMessage() narrows it.
      (err: unknown) => {
        // Surface the failure and re-mark the row dirty: the next write retries.
        this.#enqueueError = errorMessage(err);
        if (payload.kind === "session.upsert") {
          this.#pendingThreads.delete(payload.id);
          this.#dirtyThreads.add(payload.id);
        } else if (payload.kind === "session.delete") {
          this.#pendingDeletes.delete(payload.id);
        } else {
          this.#dirtyProgress.add(payload.surah);
        }
      },
    );
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
    const isCurrent = (): boolean =>
      this.#registered === name && this.#accountOwnerId() === ownerId;
    const domain = createSessionsDomain(this.#auth, this.#state, ownerId, isCurrent, () => {
      if (!isCurrent()) return;
      this.#unregisterCurrent();
      this.#state.notifySessionLost?.();
    });
    const adapter: SyncDomain<SessionMutation, SessionSnapshot> = {
      name: domain.name,
      sync: (mutations, signal) => domain.sync(mutations, signal),
      applyServer: (state, drained) => {
        if (isCurrent()) this.applyServer(state, drained);
      },
    };
    this.#register(adapter);
    this.#registered = name;
    this.#engine.start();
    // Anything written before registration (late auth probe, absorb) goes now.
    this.#push();
    void this.#engine.flush(name).catch(() => undefined);
  }
}

export const sessions: SessionsStore = new SessionsStore();
