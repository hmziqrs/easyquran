/**
 * General-purpose offline sync engine contracts.
 *
 * A "domain" (e.g. bookmarks) owns its wire transport and its local store; the
 * engine owns queueing, ordering, retry/backoff, and status. Connectivity is
 * intentionally NOT part of SyncStatus — consumers read the OnlineStore
 * ($lib/offline/online.svelte) separately.
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
  /** Per-domain monotonic sequence number; defines FIFO replay order. */
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
  /** Apply the pulled server state to the domain's local store. */
  applyServer(state: S): void;
}

export type SyncPhase = "idle" | "syncing" | "error";

export interface SyncStatus {
  readonly phase: SyncPhase;
  readonly pending: number;
  readonly lastSyncAt: number | null;
  readonly lastError: string | null;
}
