// Web-origin limiter for translated reader SSR. Process-local fixed windows:
// the app runs as a single instance (deploy rules), so no shared store.
//
// Every ceiling is env-driven and OFF (unset) by default. Owner sets values at
// >= 4x measured peak (O4). Until then a reader is never blocked by this layer.

export interface SsrLimitConfig {
  /** Per-IP ceiling per minute for ordinary addresses. null = off. */
  readonly perIp: number | null;
  /** Lower per-IP ceiling for addresses matching a shared prefix. null = off. */
  readonly perSharedIp: number | null;
  /** Textual IP prefixes treated as shared (CGNAT, corporate, carrier). */
  readonly sharedIpPrefixes: readonly string[];
  /** Global ceiling per minute across all addresses; backstop below API 600/min. null = off. */
  readonly global: number | null;
}

const WINDOW_MS = 60_000;
const MAX_TRACKED_KEYS = 10_000;
const LOGGED_IN_COOKIE = /(?:^|;\s*)ruxlog\.auth=1(?:;|$)/u;
const SESSION_COOKIE = /(?:^|;\s*)ruxlog\.sid=[^;]+/u;

function parseLimit(raw: string | undefined, name: string): number | null {
  if (raw === undefined || raw.trim() === "") return null;
  const value = raw.trim();
  if (!/^\d+$/u.test(value) || Number(value) < 1) {
    throw new Error(`[ssr-limiter] invalid ${name}: ${raw}`);
  }
  return Number(value);
}

export function parseSsrLimitConfig(
  env: Readonly<Record<string, string | undefined>>,
): SsrLimitConfig {
  return {
    perIp: parseLimit(env.WEB_SSR_IP_PER_MINUTE, "WEB_SSR_IP_PER_MINUTE"),
    perSharedIp: parseLimit(env.WEB_SSR_SHARED_IP_PER_MINUTE, "WEB_SSR_SHARED_IP_PER_MINUTE"),
    sharedIpPrefixes: (env.WEB_SSR_SHARED_IP_PREFIXES ?? "")
      .split(",")
      .map((prefix) => prefix.trim())
      .filter((prefix) => prefix !== ""),
    global: parseLimit(env.WEB_SSR_GLOBAL_PER_MINUTE, "WEB_SSR_GLOBAL_PER_MINUTE"),
  };
}

/** Translated reader paths: `/t/**`, `/ar/t/**`, and `/{surah}/t/**`. */
export function isTranslatedReaderPath(pathname: string): boolean {
  const path = pathname.replace(/^\/ar(?=\/)/u, "");
  return path.startsWith("/t/") || /^\/[^/]+\/t\//u.test(path);
}

export function isHealthPath(pathname: string): boolean {
  return (
    pathname === "/healthz" || pathname.endsWith("/health") || pathname.endsWith("/health/ready")
  );
}

/**
 * Logged-in readers skip the per-IP class only. Presence of the marker AND the
 * session cookie is not proof of a session: both are client-set. The global
 * ceiling still applies to them, so a forged cookie cannot exceed it.
 */
export function hasLoggedInMarker(cookieHeader: string | null): boolean {
  if (cookieHeader === null) return false;
  return LOGGED_IN_COOKIE.test(cookieHeader) && SESSION_COOKIE.test(cookieHeader);
}

export interface LimitDecision {
  readonly retryAfterSecs: number;
}

/** Fixed-window counter keyed by string. Bounded: expired windows are swept. */
export class FixedWindowCounter {
  readonly #windows = new Map<string, { start: number; count: number }>();

  /** Returns null when allowed, or the retry-after seconds when blocked. */
  hit(key: string, max: number, now: number): LimitDecision | null {
    const current = this.#windows.get(key);
    if (current === undefined || now - current.start >= WINDOW_MS) {
      if (this.#windows.size >= MAX_TRACKED_KEYS) this.#sweep(now);
      this.#windows.set(key, { start: now, count: 1 });
      return null;
    }
    if (current.count >= max) {
      return { retryAfterSecs: Math.max(1, Math.ceil((current.start + WINDOW_MS - now) / 1000)) };
    }
    current.count += 1;
    return null;
  }

  #sweep(now: number): void {
    for (const [key, window] of this.#windows) {
      if (now - window.start >= WINDOW_MS) this.#windows.delete(key);
    }
  }
}

export class SsrMissLimiter {
  readonly #config: SsrLimitConfig;
  readonly #counter = new FixedWindowCounter();
  readonly #now: () => number;

  constructor(config: SsrLimitConfig, now: () => number = Date.now) {
    this.#config = config;
    this.#now = now;
  }

  isShared(ip: string): boolean {
    return this.#config.sharedIpPrefixes.some((prefix) => ip.startsWith(prefix));
  }

  /**
   * Order: global backstop first, then the per-IP class. Shared addresses take
   * the lower class when configured; otherwise the ordinary class applies.
   */
  check(ip: string, loggedIn: boolean): LimitDecision | null {
    const now = this.#now();
    if (this.#config.global !== null) {
      const blocked = this.#counter.hit("global", this.#config.global, now);
      if (blocked !== null) return blocked;
    }
    if (loggedIn) return null;
    const shared = this.isShared(ip);
    const max = shared ? this.#config.perSharedIp : this.#config.perIp;
    if (max === null) return null;
    return this.#counter.hit(`${shared ? "shared" : "ip"}:${ip}`, max, now);
  }
}
