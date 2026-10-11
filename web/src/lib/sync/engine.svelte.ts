import { browser } from "$app/env";

import { trailingDebounce, type Debounced } from "#lib/storage/index.js";

import { createOutbox, type Outbox } from "./outbox";
import {
  SyncPausedError,
  type SyncDomain,
  type SyncMutation,
  type SyncPhase,
  type SyncRoundResult,
  type SyncStatus,
} from "./types";

/**
 * A domain with its payload/state generics erased. `SyncDomain` declares its
 * members with method syntax (bivariant), so every concrete
 * `SyncDomain<P, S>` is assignable to this type.
 */
export type RegisteredSyncDomain = SyncDomain<unknown, unknown>;

const FLUSH_BATCH = 100;
const DEFAULT_FLUSH_DELAY_MS = 2_000;
const RETRY_BASE_MS = 2_000;
const RETRY_CAP_MS = 60_000;
const RETRY_JITTER_FRACTION = 0.25;
const ONLINE_POLL_MS = 5_000;
const PERIODIC_FLUSH_MS = 30_000;

/** Exponential retry delay: 2s, 4s, 8s, 16s, 32s, capped at 60s. */
export function syncRetryDelayMs(failures: number): number {
  const bounded = Math.max(1, Math.min(failures, 6));
  return Math.min(RETRY_BASE_MS * 2 ** (bounded - 1), RETRY_CAP_MS);
}

function defaultOnline(): boolean {
  return browser ? navigator.onLine : false;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter is the boundary that narrows it to a displayable message string.
function messageOf(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
}

/** #drainDomain outcome: "failed" keeps the queue and backs off, "paused" skips the domain for this round without penalty. */
type DrainOutcome = "drained" | "failed" | "paused";

export interface SyncEngineOptions {
  readonly outbox?: Outbox;
  readonly domains?: RegisteredSyncDomain[];
  readonly online?: () => boolean;
  readonly flushDelayMs?: number;
}

/**
 * Offline-first sync engine: durable outbox, optimistic enqueue, ordered FIFO
 * replay per domain, retry with exponential backoff + jitter, reactive status.
 * Failed domains never block healthy ones, and failed mutations stay queued
 * (FIFO preserved) until a later round succeeds. No hard failure cap — data
 * stays queued forever; only `lastError` surfaces the problem.
 */
export class SyncEngine {
  #outbox: Outbox;
  #domains: RegisteredSyncDomain[] = [];
  #isOnline: () => boolean;
  #flushDebounce: Debounced;

  #phase = $state<SyncPhase>("idle");
  #pending = $state(0);
  #pendingByDomain = $state.raw<Readonly<Record<string, number>>>({});
  #lastSyncAt = $state<number | null>(null);
  #lastError = $state<string | null>(null);

  #running: Promise<void> | null = null;
  #coalesced = false;
  #activeDomainFilter: string | undefined;
  #coalescedDomainFilter: string | undefined;
  #controllers = new Map<string, AbortController>();
  #failures = new Map<string, number>();
  #retryTimer: ReturnType<typeof setTimeout> | null = null;
  #started = false;
  #wasOnline = false;

  constructor(opts: SyncEngineOptions = {}) {
    this.#outbox = opts.outbox ?? createOutbox();
    this.#domains = [...(opts.domains ?? [])];
    this.#isOnline = opts.online ?? defaultOnline;
    this.#flushDebounce = trailingDebounce(() => {
      void this.flush();
    }, opts.flushDelayMs ?? DEFAULT_FLUSH_DELAY_MS);
  }

  get phase(): SyncPhase {
    return this.#phase;
  }
  get pending(): number {
    return this.#pending;
  }
  pendingFor(domain: string): number {
    return this.#pendingByDomain[domain] ?? 0;
  }
  get lastSyncAt(): number | null {
    return this.#lastSyncAt;
  }
  get lastError(): string | null {
    return this.#lastError;
  }
  get status(): SyncStatus {
    return {
      phase: this.#phase,
      pending: this.#pending,
      lastSyncAt: this.#lastSyncAt,
      lastError: this.#lastError,
    };
  }

  /** Late domain binding (domains may register after engine creation). */
  register(domain: RegisteredSyncDomain): void {
    this.#controllers.get(domain.name)?.abort();
    const existing = this.#domains.findIndex((d) => d.name === domain.name);
    if (existing >= 0) this.#domains[existing] = domain;
    else this.#domains.push(domain);
  }

  unregister(name: string): void {
    this.#controllers.get(name)?.abort();
    this.#domains = this.#domains.filter((domain) => domain.name !== name);
    this.#failures.delete(name);
    if (this.#domains.length === 0) {
      this.#cancelRetry();
      this.#phase = "idle";
      this.#lastError = null;
    }
  }

  /** Persist a mutation before resolving, then schedule a debounced flush. */
  async enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>> {
    const mutation = await this.#outbox.enqueue(domain, payload);
    this.#pending += 1;
    this.#pendingByDomain = {
      ...this.#pendingByDomain,
      [domain]: this.pendingFor(domain) + 1,
    };
    this.#flushDebounce.schedule();
    return mutation;
  }

  /** Replay a domain or all domains; concurrent calls coalesce into a later pass. */
  flush(domain?: string): Promise<void> {
    if (this.#running !== null) {
      this.#coalesced = true;
      if (domain !== this.#activeDomainFilter) this.#coalescedDomainFilter = undefined;
      return this.#running;
    }
    const run = this.#drain(domain);
    this.#running = run;
    const settle = (): void => {
      if (this.#running === run) this.#running = null;
    };
    void run.then(settle, settle);
    return run;
  }

  /** Load persisted queue depth into `pending` (call once at boot / on register). */
  async hydrate(): Promise<void> {
    await this.#resyncPending();
  }

  async #resyncPending(): Promise<void> {
    try {
      const mutations = await this.#outbox.all();
      const counts: Record<string, number> = {};
      for (const mutation of mutations)
        counts[mutation.domain] = (counts[mutation.domain] ?? 0) + 1;
      this.#pending = mutations.length;
      this.#pendingByDomain = counts;
    } catch {
      return;
    }
  }

  /**
   * Wire the external flush triggers: online-poll transition (false -> true),
   * window online/offline events, document visibilitychange, and a periodic
   * round while online (push queued work, or one pull when idle). Returns a
   * teardown that removes every listener and timer. No-ops (with a noop
   * teardown) outside the browser.
   */
  start(): () => void {
    if (!browser || this.#started) return () => {};
    this.#started = true;
    this.#wasOnline = this.#isOnline();
    const teardowns: Array<() => void> = [];

    const onConnectivity = (): void => {
      const nowOnline = this.#isOnline();
      if (!this.#wasOnline && nowOnline) {
        this.#failures.clear();
        void this.flush();
      }
      this.#wasOnline = nowOnline;
    };

    const onlinePoll = setInterval(onConnectivity, ONLINE_POLL_MS);
    teardowns.push(() => clearInterval(onlinePoll));

    // Periodic round while online: drains queued mutations and — with an empty
    // queue — pulls once (pull-on-reconnect cadence; 1 req/domain/30s stays far
    // inside the API's 100 req/60s limit).
    const periodic = setInterval(() => {
      if (this.#isOnline()) void this.flush();
    }, PERIODIC_FLUSH_MS);
    teardowns.push(() => clearInterval(periodic));

    const onVisibility = (): void => {
      if (document.visibilityState === "visible" && this.#isOnline()) void this.flush();
    };
    document.addEventListener("visibilitychange", onVisibility);
    teardowns.push(() => document.removeEventListener("visibilitychange", onVisibility));

    window.addEventListener("online", onConnectivity);
    window.addEventListener("offline", onConnectivity);
    teardowns.push(() => {
      window.removeEventListener("online", onConnectivity);
      window.removeEventListener("offline", onConnectivity);
    });

    if (this.#wasOnline && this.#hasRegisteredPending()) void this.flush();

    return () => {
      for (const teardown of teardowns) teardown();
      this.#flushDebounce.cancel();
      this.#cancelRetry();
      this.#started = false;
    };
  }

  async #drain(domainFilter: string | undefined): Promise<void> {
    let filter = domainFilter;
    do {
      this.#coalesced = false;
      this.#activeDomainFilter = filter;
      this.#coalescedDomainFilter = filter;
      await this.#flushPass(filter);
      await this.#resyncPending();
      filter = this.#coalescedDomainFilter;
    } while (this.#coalesced);
  }

  async #flushPass(domainFilter: string | undefined): Promise<void> {
    // Targets first: an offline branch must only penalize the domains this
    // flush actually targets, and a flush naming no registered domain (and no
    // filter) is a full no-op — no phase, lastError, or retry mutation.
    const targets = this.#targets(domainFilter);
    if (targets.length === 0) return;
    this.#cancelRetry();
    if (!this.#isOnline()) {
      this.#phase = "error";
      this.#lastError = "offline";
      // Offline fails the flush's target domains alike: grow each one's backoff
      // (reset on success / online transition as elsewhere) so the retry timer
      // spaces out instead of refiring every ~2s with zero network calls.
      for (const domain of targets) {
        this.#failures.set(domain.name, (this.#failures.get(domain.name) ?? 0) + 1);
      }
      this.#scheduleRetry();
      return;
    }
    this.#phase = "syncing";
    let failed = false;
    for (const domain of targets) {
      const outcome = await this.#drainDomain(domain);
      if (!this.#domains.includes(domain)) continue;
      if (outcome === "failed") {
        this.#failures.set(domain.name, (this.#failures.get(domain.name) ?? 0) + 1);
        failed = true;
      } else if (outcome === "drained") {
        this.#failures.delete(domain.name);
      }
    }
    if (failed) {
      // Keep lastError from the failing domain; its mutations stay queued.
      this.#phase = "error";
      this.#scheduleRetry();
    } else {
      this.#phase = "idle";
      this.#lastError = null;
    }
  }

  #targets(domainFilter: string | undefined): RegisteredSyncDomain[] {
    if (domainFilter === undefined) return [...this.#domains];
    return this.#domains.filter((d) => d.name === domainFilter);
  }

  async #drainDomain(domain: RegisteredSyncDomain): Promise<DrainOutcome> {
    const controller = new AbortController();
    const isCurrent = (): boolean => !controller.signal.aborted && this.#domains.includes(domain);
    this.#controllers.set(domain.name, controller);
    try {
      let pushedAny = false;
      for (;;) {
        if (!isCurrent()) return "paused";
        let batch: SyncMutation[];
        try {
          batch = await this.#outbox.take(domain.name, FLUSH_BATCH);
        } catch (err) {
          this.#lastError = messageOf(err);
          return "failed";
        }
        const pullOnly = batch.length === 0;
        if (pullOnly && pushedAny) return "drained";
        if (!isCurrent()) return "paused";
        let result: SyncRoundResult<unknown>;
        try {
          result = await domain.sync(batch, controller.signal);
        } catch (err) {
          if (!isCurrent() || err instanceof SyncPausedError) return "paused";
          this.#lastError = messageOf(err);
          return "failed";
        }
        if (!isCurrent()) return "paused";
        try {
          if (!pullOnly) {
            await this.#outbox.remove(
              domain.name,
              batch.map((m) => m.id),
            );
            this.#pending = Math.max(0, this.#pending - batch.length);
            this.#pendingByDomain = {
              ...this.#pendingByDomain,
              [domain.name]: Math.max(0, this.pendingFor(domain.name) - batch.length),
            };
          }
          if (!isCurrent()) return "paused";
          this.#lastSyncAt = Date.now();
          this.#lastError = null;
        } catch (err) {
          this.#lastError = messageOf(err);
          return "failed";
        }
        try {
          domain.applyServer(result.state, batch);
        } catch (err) {
          this.#lastError = messageOf(err);
          return "failed";
        }
        if (pullOnly) return "drained";
        pushedAny = true;
      }
    } finally {
      if (this.#controllers.get(domain.name) === controller) this.#controllers.delete(domain.name);
    }
  }

  /** Durable wipe of one domain's queue (e.g. its account signed out); `pending` follows. */
  async clear(domain: string): Promise<void> {
    const removed = await this.#outbox.clear(domain);
    this.#pending = Math.max(0, this.#pending - removed);
    this.#pendingByDomain = {
      ...this.#pendingByDomain,
      [domain]: Math.max(0, this.pendingFor(domain) - removed),
    };
  }

  #scheduleRetry(): void {
    this.#cancelRetry();
    if (!this.#hasRegisteredPending()) return;
    let worst = 0;
    for (const count of this.#failures.values()) worst = Math.max(worst, count);
    const base = syncRetryDelayMs(Math.max(1, worst));
    const jitter = Math.round(base * RETRY_JITTER_FRACTION * Math.random());
    this.#retryTimer = setTimeout(() => {
      this.#retryTimer = null;
      void this.flush();
    }, base + jitter);
  }

  #hasRegisteredPending(): boolean {
    return this.#domains.some((domain) => this.pendingFor(domain.name) > 0);
  }

  #cancelRetry(): void {
    if (this.#retryTimer === null) return;
    clearTimeout(this.#retryTimer);
    this.#retryTimer = null;
  }
}

export function createSyncEngine(opts: SyncEngineOptions = {}): SyncEngine {
  return new SyncEngine(opts);
}
