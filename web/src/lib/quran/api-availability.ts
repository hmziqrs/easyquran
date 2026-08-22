import { FetchHttpError, FetchTimeoutError, MalformedDataError } from "./fetch";

export const QURAN_API_COOLDOWN_MS = 30_000;
export const QURAN_API_SERVER_FAILURE_THRESHOLD = 2;

export class QuranApiUnavailableError extends Error {
  constructor() {
    super("quran api temporarily unavailable");
    this.name = "QuranApiUnavailableError";
  }
}

interface QuranApiAvailabilityOptions {
  readonly cooldownMs?: number;
  readonly serverFailureThreshold?: number;
  readonly now?: () => number;
}

type AvailabilityState = "closed" | "open" | "half-open";

// eslint-disable-next-line anti-slop/no-unknown-parameters -- catches receive opaque rejection reasons; this classifier narrows only known DOM abort errors
function isCallerAbort(error: unknown, signal?: AbortSignal): boolean {
  if (signal?.aborted) return true;
  return error instanceof DOMException && error.name === "AbortError";
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- catches receive opaque rejection reasons; this classifier recognizes validated fetch-domain errors
function isNeutralFailure(error: unknown): boolean {
  if (error instanceof MalformedDataError || error instanceof SyntaxError) return true;
  return error instanceof FetchHttpError && error.status < 500;
}

/**
 * Passive browser-side availability memory. Real read/search requests are probes;
 * no separate health request is generated.
 */
export class QuranApiAvailability {
  readonly #cooldownMs: number;
  readonly #serverFailureThreshold: number;
  readonly #now: () => number;
  #state: AvailabilityState = "closed";
  #openUntil = 0;
  #serverFailures = 0;

  constructor(options: QuranApiAvailabilityOptions = {}) {
    this.#cooldownMs = options.cooldownMs ?? QURAN_API_COOLDOWN_MS;
    this.#serverFailureThreshold =
      options.serverFailureThreshold ?? QURAN_API_SERVER_FAILURE_THRESHOLD;
    this.#now = options.now ?? Date.now;
  }

  async run<T>(operation: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    const halfOpenProbe = this.#acquire();
    try {
      const value = await operation();
      this.reset();
      return value;
    } catch (error) {
      this.#recordFailure(error, signal, halfOpenProbe);
      throw error;
    }
  }

  reset(): void {
    this.#state = "closed";
    this.#openUntil = 0;
    this.#serverFailures = 0;
  }

  #acquire(): boolean {
    if (this.#state === "closed") return false;
    if (this.#state === "half-open") throw new QuranApiUnavailableError();
    if (this.#now() < this.#openUntil) throw new QuranApiUnavailableError();
    this.#state = "half-open";
    return true;
  }

  // eslint-disable-next-line anti-slop/no-unknown-parameters -- run() catches opaque operation failures and parses them through domain error classes here
  #recordFailure(error: unknown, signal: AbortSignal | undefined, halfOpenProbe: boolean): void {
    if (isCallerAbort(error, signal) || isNeutralFailure(error)) {
      this.reset();
      return;
    }
    if (error instanceof FetchHttpError) {
      this.#serverFailures++;
      if (halfOpenProbe || this.#serverFailures >= this.#serverFailureThreshold) this.#open();
      return;
    }
    if (error instanceof FetchTimeoutError || error instanceof Error) {
      this.#open();
      return;
    }
    this.#open();
  }

  #open(): void {
    this.#state = "open";
    this.#openUntil = this.#now() + this.#cooldownMs;
  }
}

export const quranApiAvailability = new QuranApiAvailability();
