import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { readingAnalytics } from "#lib/analytics/reading-analytics.js";
import { decodeReader } from "#lib/stores/reader-persistence.svelte.js";
import { readJSON, trailingDebounce, writeJSON } from "#lib/storage/index.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  type RegisteredSyncDomain,
  type SyncDomain,
  type SyncMutation,
} from "#lib/sync/index.js";

import {
  createSessionsDomain,
  sessionsDomainFor,
  type SessionsAuthLike,
  type SessionsAuthStateLike,
} from "./domain.svelte";
import {
  LOCAL_DEFAULT_SESSION_ID,
  SESSIONS_SCHEMA_VERSION,
  SESSION_NAME_MAX,
  accountDefaultSessionId,
  decodeSessionState,
  decodeSessionsEnvelope,
  defaultSessionFor,
  emptySessionState,
  newSessionId,
  progressTs,
  type ReadingSession,
  type SessionKind,
  type SessionMutation,
  type SessionReaderState,
  type SessionSnapshot,
  type SessionsEnvelope,
} from "./schema.svelte";

/** Engine surface the store depends on (mirrors BookmarksStore.SyncEngineLike). */
export interface SessionsEngineLike {
  enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>>;
  flush(domain?: string): Promise<void>;
  start(): () => void;
}

export interface SessionsStoreDeps {
  readonly engine?: SessionsEngineLike;
  readonly auth?: SessionsAuthLike;
  /** Auth status source; authState satisfies this structurally. */
  readonly state?: SessionsAuthStateLike & { notifySessionLost?(): void };
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

/**
 * Per-owner reading-session store (Feature 4). Owns the durable key
 * `easyquran.sessions.{owner}` (owner = user id when signed in, `local`
 * otherwise) and never touches the reader blob — the reader session keeps
 * core.s as its live mirror and reports every position write through
 * `recordActive`. Positions + progress sync through the per-owner
 * `sessions.user:{id}` outbox partition (§9.2); recents stay device-local.
 */

const LOCAL_OWNER = "local";
/** Mirror of reader-persistence's module-private STORAGE_KEY — migration reads the legacy blob. */
const READER_BLOB_KEY = "easyquran.reader";
/** Position/progress pushes coalesce: scrolling marks many verses in seconds. */
const PUSH_DEBOUNCE_MS = 4000;

function sessionsKeyFor(owner: string): string {
  return `easyquran.sessions.${owner}`;
}

/** EC-35 deterministic order: (updatedAt, id) — same rule as the server's snapshot. */
function byUpdatedAtThenId(a: ReadingSession, b: ReadingSession): number {
  if (a.updatedAt !== b.updatedAt) return a.updatedAt < b.updatedAt ? -1 : 1;
  return a.id < b.id ? -1 : 1;
}

function nowIso(): string {
  return new Date().toISOString();
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter is the boundary that narrows it to a displayable message string.
function errorMessage(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- name is raw user input; this validator is its boundary check.
function validSessionName(name: string): string | null {
  const trimmed = name.trim();
  return trimmed.length > 0 && trimmed.length <= SESSION_NAME_MAX ? trimmed : null;
}

export class SessionsStore {
  #owner = LOCAL_OWNER;
  #hydrated = false;
  #envelope = $state.raw<SessionsEnvelope | null>(null);
  readonly #appliers = new Set<(state: SessionReaderState) => void>();
  readonly #engine: SessionsEngineLike;
  readonly #auth: SessionsAuthLike;
  readonly #state: SessionsAuthStateLike & { notifySessionLost?(): void };
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;
  #registered: string | null = null;
  /** Queued-but-undrained session upserts — win over the server snapshot (row-level LWW, §9.3). */
  #pendingSessions = new Map<string, ReadingSession>();
  #pendingSessionDeletes = new Set<string>();
  /** Sessions with queued position/progress — their snapshot rows are skipped until drained. */
  #pendingStateSessions = new Set<string>();
  /** Last furthestAyah pushed per `${sessionId}:${surah}` — only growth is enqueued. */
  #pushedProgress = new Map<string, number>();
  /** Last durable-enqueue failure (EC-27): surfaced for the sync line instead of silently dropping the mutation. */
  #enqueueError = $state<string | null>(null);

  constructor(deps: SessionsStoreDeps = {}) {
    this.#engine = deps.engine ?? syncEngine;
    this.#auth = deps.auth ?? authClient;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
    this.#unregister = deps.unregister ?? unregisterDomain;
    // Hydrate at construction (module-import time, before any render): first-touch
    // hydration inside a getter throws state_unsafe_mutation when a $derived reads
    // it (the /yours cold-load crash). SSR stays unhydrated and reads empty.
    if (browser) this.#ensureHydrated();
  }

  get activeId(): string | null {
    return this.#envelope?.activeId ?? null;
  }

  get activeSession() {
    const envelope = this.#envelope;
    if (envelope === null) return null;
    return envelope.sessions.find((session) => session.id === envelope.activeId) ?? null;
  }

  /** The owner's live (non-archived-first, EC-35-ordered) sessions. */
  get list(): readonly ReadingSession[] {
    return this.#envelope?.sessions ?? [];
  }

  /** Last durable-enqueue failure (EC-27), surfaced for the /yours sync line. */
  get enqueueError(): string | null {
    return this.#enqueueError;
  }

  /**
   * Register the live-mirror sync: called with the active session's state
   * whenever it is replaced wholesale (owner change, session switch, sync
   * snapshot apply), never on ordinary writes, which the reader session
   * mirrors itself.
   */
  onApplyState(fn: (state: SessionReaderState) => void): () => void {
    this.#appliers.add(fn);
    return () => {
      this.#appliers.delete(fn);
    };
  }

  /**
   * Copy the reader's freshly computed position/recents/progress into the
   * active session and persist the owner's key. Re-resolves the owner first so
   * a probe that resolved auth after boot moves the partition without an
   * explicit transition (§9.5).
   *
   * Every Feature-4 writer (openVerse/markRead + their recordProgress) reports
   * through here, so the reading-analytics emit rides this seam: a changed
   * `lastRead` (the verse the reader advanced to) is one opt-in `ayah.read`
   * event. Anchor-only writes keep `lastRead` and emit nothing, and the diff is
   * per (activeId) so an owner/session switch cannot fake a read.
   */
  recordActive(state: SessionReaderState): void {
    this.syncOwner();
    const envelope = this.#envelope;
    if (envelope === null) return;
    const activeId = envelope.activeId;
    const previous = envelope.state[activeId]?.lastRead ?? null;
    const next = state.lastRead;
    if (
      next !== null &&
      (previous === null ||
        previous.num !== next.num ||
        previous.n !== next.n ||
        (previous.sourceId ?? null) !== (next.sourceId ?? null))
    ) {
      readingAnalytics.emit({
        kind: "ayah.read",
        surah: next.num,
        ayah: next.n,
        sourceId: next.sourceId ?? null,
        seconds: null,
      });
    }
    this.#envelope = { ...envelope, state: { ...envelope.state, [activeId]: { ...state } } };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    if (this.#ownerIsAccount() && activeId !== LOCAL_DEFAULT_SESSION_ID) {
      this.#schedulePush.schedule();
    }
  }

  // --- session CRUD (local-first; every write enqueues a sync mutation) ---

  /** Make `id` active: the live mirror swaps to its state wholesale. */
  switchTo(id: string): boolean {
    this.#ensureHydrated();
    const envelope = this.#envelope;
    if (envelope === null || !envelope.sessions.some((session) => session.id === id)) return false;
    if (envelope.activeId === id) return true;
    this.#envelope = { ...envelope, activeId: id };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    this.#pushActiveToMirror();
    return true;
  }

  create(name: string, kind: SessionKind = "free"): ReadingSession | null {
    this.#ensureHydrated();
    const trimmed = validSessionName(name);
    const envelope = this.#envelope;
    if (envelope === null || trimmed === null) return null;
    const session: ReadingSession = {
      id: newSessionId(),
      name: trimmed,
      kind,
      translationId: null,
      createdAt: nowIso(),
      updatedAt: nowIso(),
      archivedAt: null,
    };
    this.#envelope = {
      ...envelope,
      activeId: session.id,
      sessions: [...envelope.sessions, session],
      state: { ...envelope.state, [session.id]: emptySessionState() },
    };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    this.#pushActiveToMirror();
    this.#enqueueSessionUpsert(session);
    return session;
  }

  /** Whole-row re-upsert (rename / archive ride the same mutation, §9.3 row-level LWW). */
  update(id: string, patch: { name?: string; archived?: boolean }): void {
    this.#ensureHydrated();
    const envelope = this.#envelope;
    const existing = envelope?.sessions.find((session) => session.id === id);
    if (envelope === null || existing === undefined) return;
    let updated: ReadingSession = { ...existing, updatedAt: nowIso() };
    if (patch.name !== undefined) {
      const trimmed = validSessionName(patch.name);
      if (trimmed === null) return;
      updated = { ...updated, name: trimmed };
    }
    if (patch.archived !== undefined) {
      updated = { ...updated, archivedAt: patch.archived ? nowIso() : null };
    }
    this.#envelope = {
      ...envelope,
      sessions: envelope.sessions.map((session) => (session.id === id ? updated : session)),
    };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    this.#enqueueSessionUpsert(updated);
  }

  remove(id: string): void {
    this.#ensureHydrated();
    const envelope = this.#envelope;
    if (envelope === null || !envelope.sessions.some((session) => session.id === id)) return;
    const sessions = envelope.sessions.filter((session) => session.id !== id);
    const state = { ...envelope.state };
    delete state[id];
    let activeId = envelope.activeId;
    if (activeId === id) {
      activeId = sessions[0]?.id ?? this.#defaultSessionIdFor(this.#owner);
      if (!sessions.some((session) => session.id === activeId)) {
        const seed = defaultSessionFor(activeId);
        sessions.push(seed);
        state[activeId] = emptySessionState();
      }
    }
    this.#envelope = { ...envelope, activeId, sessions, state };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    this.#pendingSessions.delete(id);
    if (this.#ownerIsAccount()) {
      this.#pendingSessionDeletes.add(id);
      this.#enqueue({ kind: "session.delete", id, updatedAt: nowIso() }, id);
    }
    this.#pushActiveToMirror();
  }

  // --- sync (§9.2/§9.5): per-owner partition, quarantine on logout ---

  /**
   * Auth edge (purge-hook) or a late probe that changed the resolved owner:
   * swap the in-memory view, register the account's domain, or leave the
   * partition quarantined while signed out — never destroyed (EC-14).
   */
  onAuthTransition(): void {
    this.syncOwner();
    if (!browser) return;
    const ownerId = this.#accountOwnerId();
    if (ownerId === null) return; // partition waits in IndexedDB for its owner
    this.#registerFor(ownerId);
  }

  applyServer(snapshot: SessionSnapshot, drained?: readonly SyncMutation<SessionMutation>[]): void {
    if (drained !== undefined) {
      for (const mutation of drained) {
        const payload = mutation.payload;
        if (payload.kind === "session.upsert") this.#pendingSessions.delete(payload.id);
        else if (payload.kind === "session.delete") this.#pendingSessionDeletes.delete(payload.id);
        else this.#pendingStateSessions.delete(payload.sessionId);
      }
    }
    if (!this.#hydrated || !this.#ownerIsAccount()) return;
    const envelope = this.#envelope;
    if (envelope === null) return;

    const byId = new Map<string, ReadingSession>();
    for (const session of envelope.sessions) {
      if (!this.#pendingSessionDeletes.has(session.id)) byId.set(session.id, session);
    }
    for (const session of snapshot.sessions) {
      if (!this.#pendingSessionDeletes.has(session.id)) byId.set(session.id, session);
    }
    for (const [id, session] of this.#pendingSessions) byId.set(id, session);
    // EC-35: deterministic `(updatedAt, id)` order — the server list already
    // carries it; the merged view re-sorts so every device converges.
    const sessions = [...byId.values()].sort(byUpdatedAtThenId);

    const state = { ...envelope.state } satisfies Record<string, SessionReaderState>;
    for (const row of snapshot.positions) {
      if (this.#pendingStateSessions.has(row.sessionId)) continue;
      const base = state[row.sessionId] ?? emptySessionState();
      const anchor =
        row.localPage !== null && row.ratio !== null
          ? { verseKey: `${row.surah}:${row.ayah}`, localPage: row.localPage, ratio: row.ratio }
          : null;
      state[row.sessionId] = {
        ...base,
        lastRead:
          row.sourceId !== null
            ? { num: row.surah, n: row.ayah, sourceId: row.sourceId }
            : { num: row.surah, n: row.ayah },
        lastReadAnchor: anchor,
      };
    }
    for (const row of snapshot.progress) {
      // Seed the pushed-progress ledger: an equal server row needs no re-push.
      this.#pushedProgress.set(`${row.sessionId}:${row.surah}`, row.furthestAyah);
      if (this.#pendingStateSessions.has(row.sessionId)) continue;
      const base = state[row.sessionId] ?? emptySessionState();
      const prev = base.progress[row.surah];
      // Progress never regresses (EC-05): the server row is MAX-resolved, the
      // local mirror may still hold an equal-or-higher offline value.
      const furthestAyah = prev ? Math.max(prev.furthestAyah, row.furthestAyah) : row.furthestAyah;
      const ts =
        prev && prev.furthestAyah >= row.furthestAyah ? prev.ts : progressTs(row.updatedAt);
      state[row.sessionId] = {
        ...base,
        progress: { ...base.progress, [row.surah]: { furthestAyah, ts } },
      };
    }
    for (const session of sessions) {
      if (state[session.id] === undefined) state[session.id] = emptySessionState();
    }
    let activeId = envelope.activeId;
    if (!sessions.some((session) => session.id === activeId)) {
      const fallback = this.#defaultSessionIdFor(this.#owner);
      activeId = sessions.some((session) => session.id === fallback)
        ? fallback
        : (sessions[0]?.id ?? activeId);
    }
    this.#envelope = { ...envelope, activeId, sessions, state };
    writeJSON(sessionsKeyFor(this.#owner), this.#envelope);
    this.#pushActiveToMirror();
  }

  /**
   * Re-resolve the owning account after a login/logout/expiry transition or a
   * late auth probe: swap the in-memory envelope for the new owner's key
   * (partitions are never merged across accounts, §9.5), drop the previous
   * account's registration, and push the new active session's state to the
   * live mirror.
   */
  syncOwner(): void {
    this.#ensureHydrated();
    const next = this.#resolveOwner();
    if (next === this.#owner) return;
    this.#owner = next;
    this.#unregisterCurrent();
    this.#schedulePush.cancel();
    this.#pushedProgress.clear();
    this.#pendingSessions.clear();
    this.#pendingSessionDeletes.clear();
    this.#pendingStateSessions.clear();
    this.#loadOwner(next);
    this.#pushActiveToMirror();
  }

  // Anonymous or still-unresolved auth: the device-local owner holds the data
  // (Feature 4.2 — the reader hydrates before auth resolves).
  #resolveOwner(): string {
    if (this.#state.authenticated && this.#state.user !== null) {
      return String(this.#state.user.id);
    }
    return LOCAL_OWNER;
  }

  #ownerIsAccount(): boolean {
    return this.#owner !== LOCAL_OWNER;
  }

  #accountOwnerId(): number | null {
    if (!this.#ownerIsAccount()) return null;
    const id = Number(this.#owner);
    return Number.isSafeInteger(id) && id > 0 ? id : null;
  }

  #defaultSessionIdFor(owner: string): string {
    return owner === LOCAL_OWNER
      ? LOCAL_DEFAULT_SESSION_ID
      : accountDefaultSessionId(Number(owner));
  }

  #pushActiveToMirror(): void {
    const envelope = this.#envelope;
    if (envelope === null) return;
    const state = envelope.state[envelope.activeId] ?? emptySessionState();
    for (const fn of this.#appliers) fn(state);
  }

  #ensureHydrated(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    // First touch may happen after auth already resolved (fast probe, layout
    // mount after hydrate): start at the resolved owner, not local-then-swap.
    this.#owner = this.#resolveOwner();
    this.#loadOwner(this.#owner);
  }

  #loadOwner(owner: string): void {
    const key = sessionsKeyFor(owner);
    const stored = decodeSessionsEnvelope(readJSON(key));
    if (stored !== undefined) {
      this.#envelope = stored;
      return;
    }
    // First boot for this owner; the key's existence is the once-marker. The
    // v4 reader blob's position fields seed the local default session exactly
    // once (Feature 4.2); an account starts from its deterministic default
    // session id so devices converge (EC-22).
    const id = this.#defaultSessionIdFor(owner);
    this.#envelope = {
      v: SESSIONS_SCHEMA_VERSION,
      activeId: id,
      sessions: [defaultSessionFor(id)],
      state: {
        [id]:
          owner === LOCAL_OWNER
            ? decodeSessionState(decodeReader(readJSON(READER_BLOB_KEY)))
            : emptySessionState(),
      },
    };
    writeJSON(key, this.#envelope);
  }

  // --- sync plumbing ---

  #schedulePush = trailingDebounce(() => this.#pushActiveState(), PUSH_DEBOUNCE_MS);

  /** Coalesced position+progress push of the active session (debounced by recordActive). */
  #pushActiveState(): void {
    if (!this.#hydrated || !this.#ownerIsAccount()) return;
    const envelope = this.#envelope;
    if (envelope === null) return;
    const ownerId = this.#accountOwnerId();
    if (ownerId === null || this.#registered !== sessionsDomainFor(ownerId)) return;
    const sessionId = envelope.activeId;
    const state = envelope.state[sessionId];
    if (state === undefined) return;
    const updatedAt = nowIso();
    if (state.lastRead !== null) {
      const anchor = state.lastReadAnchor;
      this.#enqueue(
        {
          kind: "position.upsert",
          sessionId,
          surah: state.lastRead.num,
          ayah: state.lastRead.n,
          localPage: anchor?.localPage ?? null,
          ratio: anchor?.ratio ?? null,
          sourceId: state.lastRead.sourceId ?? null,
          updatedAt,
        },
        sessionId,
      );
    }
    for (const [surah, entry] of Object.entries(state.progress)) {
      const num = Number(surah);
      const key = `${sessionId}:${surah}`;
      if (this.#pushedProgress.get(key) === entry.furthestAyah) continue;
      this.#enqueue(
        {
          kind: "progress.upsert",
          sessionId,
          surah: num,
          furthestAyah: entry.furthestAyah,
          updatedAt,
        },
        sessionId,
        // Record only after the enqueue is durable — a rejection must not suppress the re-push.
        () => this.#pushedProgress.set(key, entry.furthestAyah),
      );
    }
  }

  #enqueueSessionUpsert(session: ReadingSession): void {
    if (!this.#ownerIsAccount()) return;
    this.#pendingSessions.set(session.id, session);
    this.#pendingSessionDeletes.delete(session.id);
    this.#enqueue(
      {
        kind: "session.upsert",
        id: session.id,
        name: session.name,
        sessionKind: session.kind,
        translationId: session.translationId,
        archivedAt: session.archivedAt,
        updatedAt: session.updatedAt,
      },
      session.id,
    );
  }

  #enqueue(payload: SessionMutation, sessionId: string, onDurable?: () => void): void {
    const ownerId = this.#accountOwnerId();
    if (ownerId === null) return;
    if (payload.kind === "position.upsert" || payload.kind === "progress.upsert") {
      this.#pendingStateSessions.add(sessionId);
    }
    void this.#engine.enqueue(sessionsDomainFor(ownerId), payload).then(
      () => {
        this.#enqueueError = null;
        onDurable?.();
      },
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); errorMessage() narrows it to a displayable string.
      (err: unknown) => {
        // EC-27: surface the failure; drop the overlay so the next snapshot applies
        // and a later write re-enqueues (the reader mirror holds the state — never lost).
        this.#enqueueError = errorMessage(err);
        if (payload.kind === "session.upsert") this.#pendingSessions.delete(payload.id);
        else if (payload.kind === "session.delete") this.#pendingSessionDeletes.delete(payload.id);
        else this.#pendingStateSessions.delete(payload.sessionId);
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
    void this.#engine.flush(name).catch(() => undefined);
    // Registration may lag position writes (late probe): push the active
    // session's durable state once so nothing recorded before the domain
    // existed is left unsynced.
    this.#schedulePush.schedule();
  }
}

export const sessions: SessionsStore = new SessionsStore();
