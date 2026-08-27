import { browser } from "$app/environment";
import { trailingDebounce, type Debounced } from "$lib/storage";
import { createOutbox, type Outbox } from "./outbox";
import type { SyncDomain, SyncMutation, SyncPhase, SyncStatus } from "./types";

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
  #lastSyncAt = $state<number | null>(null);
  #lastError = $state<string | null>(null);

  #running: Promise<void> | null = null;
  #coalesced = false;
  #consecutiveFailures = 0;
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
    const existing = this.#domains.findIndex((d) => d.name === domain.name);
    if (existing >= 0) this.#domains[existing] = domain;
    else this.#domains.push(domain);
  }

  /**
   * Queue a mutation (durable before the returned promise resolves) and return
   * it so the caller can apply it optimistically. Schedules a debounced flush.
   */
  async enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>> {
    const mutation = await this.#outbox.enqueue(domain, payload);
    this.#pending += 1;
    this.#flushDebounce.schedule();
    return mutation;
  }

  /**
   * Replay queued mutations, all domains or just `domain`. Re-entrant calls are
   * coalesced into the in-flight round (which then runs one extra pass).
   */
  flush(domain?: string): Promise<void> {
    if (this.#running !== null) {
      this.#coalesced = true;
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
    this.#pending = (await this.#outbox.all()).length;
  }

  /**
   * Wire the external flush triggers: online-poll transition (false -> true),
   * window online/offline events, document visibilitychange, and a periodic
   * flush while online with pending work. Returns a teardown that removes every
   * listener and timer. No-ops (with a noop teardown) outside the browser.
   */
  start(): () => void {
    if (!browser || this.#started) return () => {};
    this.#started = true;
    this.#wasOnline = this.#isOnline();
    const teardowns: Array<() => void> = [];

    const onConnectivity = (): void => {
      const nowOnline = this.#isOnline();
      if (!this.#wasOnline && nowOnline) {
        this.#consecutiveFailures = 0;
        void this.flush();
      }
      this.#wasOnline = nowOnline;
    };

    const onlinePoll = setInterval(onConnectivity, ONLINE_POLL_MS);
    teardowns.push(() => clearInterval(onlinePoll));

    const periodic = setInterval(() => {
      if (this.#isOnline() && this.#pending > 0) void this.flush();
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

    if (this.#wasOnline && this.#pending > 0) void this.flush();

    return () => {
      for (const teardown of teardowns) teardown();
      this.#flushDebounce.cancel();
      this.#cancelRetry();
      this.#started = false;
    };
  }

  async #drain(domainFilter: string | undefined): Promise<void> {
    do {
      this.#coalesced = false;
      await this.#flushPass(domainFilter);
    } while (this.#coalesced);
  }

  async #flushPass(domainFilter: string | undefined): Promise<void> {
    this.#cancelRetry();
    if (!this.#isOnline()) {
      this.#recordFailure("offline");
      return;
    }
    const targets = this.#targets(domainFilter);
    if (targets.length === 0) return;
    this.#phase = "syncing";
    let failed = false;
    for (const domain of targets) {
      if (!(await this.#drainDomain(domain))) failed = true;
    }
    if (failed) {
      // Keep lastError from the failing domain; mutations stay queued.
      this.#recordFailure(null);
    } else {
      this.#consecutiveFailures = 0;
      this.#phase = "idle";
      this.#lastError = null;
    }
  }

  #targets(domainFilter: string | undefined): RegisteredSyncDomain[] {
    if (domainFilter === undefined) return [...this.#domains];
    return this.#domains.filter((d) => d.name === domainFilter);
  }

  /** Drain one domain in FIFO batches until empty; false = it failed (queue kept). */
  async #drainDomain(domain: RegisteredSyncDomain): Promise<boolean> {
    for (;;) {
      const batch = await this.#outbox.take(domain.name, FLUSH_BATCH);
      if (batch.length === 0) return true;
      try {
        const result = await domain.sync(batch);
        await this.#outbox.remove(domain.name, batch.map((m) => m.id));
        this.#pending = Math.max(0, this.#pending - batch.length);
        this.#lastSyncAt = Date.now();
        this.#lastError = null;
        domain.applyServer(result.state);
      } catch (err) {
        this.#lastError = err instanceof Error && err.message ? err.message : String(err);
        return false;
      }
    }
  }

  #recordFailure(message: string | null): void {
    this.#phase = "error";
    if (message !== null) this.#lastError = message;
    this.#consecutiveFailures += 1;
    this.#scheduleRetry();
  }

  #scheduleRetry(): void {
    this.#cancelRetry();
    if (this.#pending <= 0) return;
    const base = syncRetryDelayMs(this.#consecutiveFailures);
    const jitter = Math.round(base * RETRY_JITTER_FRACTION * Math.random());
    this.#retryTimer = setTimeout(() => {
      this.#retryTimer = null;
      void this.flush();
    }, base + jitter);
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
