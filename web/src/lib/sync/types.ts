/**
 * General-purpose offline sync engine contracts.
 *
 * A "domain" (e.g. bookmarks) owns its wire transport and its local store; the
 * engine owns queueing, ordering, retry/backoff, and status. Connectivity is
 * intentionally NOT part of SyncStatus — consumers read the OnlineStore
 * (#lib/offline/online.svelte.js) separately.
 */

/** Identifier of a queued mutation (crypto.randomUUID() when available). */
export type SyncMutationId = string;

/**
 * One durable local mutation waiting to be replayed to the server.
 * `payload` is domain-owned and opaque to the engine; it must be
 * structured-cloneable because the outbox persists it in IndexedDB.
 */
export interface SyncMutation<P = unknown> {
  readonly id: SyncMutationId;
  readonly domain: string;
  /** Durable insertion key allocated by the storage (FIFO within a domain = ascending seq). */
  readonly seq: number;
  readonly payload: P;
  readonly queuedAt: number;
}

/** Result of one domain-owned push(+pull) round over a batch of mutations. */
export interface SyncRoundResult<S> {
  readonly applied: number;
  readonly skipped?: number;
  readonly state: S;
}

/**
 * A feature that syncs through the engine. Method syntax keeps `sync`/
 * `applyServer` bivariant, so a concrete `SyncDomain<P, S>` registers as the
 * engine-erased `SyncDomain<unknown, unknown>`.
 */
export interface SyncDomain<P = unknown, S = unknown> {
  readonly name: string;
  /** Push `mutations` (in FIFO order) and return the pulled server state. */
  sync(mutations: SyncMutation<P>[], signal?: AbortSignal): Promise<SyncRoundResult<S>>;
  /**
   * Apply the pulled server state to the domain's local store. `drained` is the
   * batch whose mutations were just removed from the queue (post-removal call
   * order), letting the domain retire optimistic overlay entries for those
   * entities instead of dropping every in-flight local edit.
   */
  applyServer(state: S, drained?: SyncMutation<P>[]): void;
}

/**
 * Thrown by a domain's `sync` when it must not run right now (e.g. its account
 * signed out). The engine treats it as a skip: no failure count, no backoff
 * escalation, and the domain's drain loop stops for that round. The throw must
 * happen before any network request.
 */
export class SyncPausedError extends Error {
  constructor(message = "sync paused") {
    super(message);
    this.name = "SyncPausedError";
  }
}

/**
 * Thrown by a domain's `sync` when the server rejected the batch as invalid
 * (validation 4xx — e.g. HTTP 400 `InvalidInput`). A durable verdict, unlike a
 * network/5xx error, so the engine counts it on a separate ledger and
 * dead-letters the batch head after enough consecutive rejections so the rest
 * of the queue can drain (EC-29). Never thrown for offline rounds,
 * `SyncPausedError`, or account-mismatch rejections.
 */
export class SyncRejectedError extends Error {
  constructor(message = "sync rejected by server") {
    super(message);
    this.name = "SyncRejectedError";
  }
}

export type SyncPhase = "idle" | "syncing" | "error";

export interface SyncStatus {
  readonly phase: SyncPhase;
  readonly pending: number;
  readonly lastSyncAt: number | null;
  readonly lastError: string | null;
  /** True once the durable outbox degraded to in-memory storage (EC-25) — queued changes may not survive a reload. */
  readonly degraded?: boolean;
  /** Mutations moved to the dead-letter store by this engine (EC-29). */
  readonly deadLettered?: number;
}
