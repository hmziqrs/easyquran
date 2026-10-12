import { browser } from "$app/env";

import {
  authClient,
  type AuthClient,
  type SessionProbeResult,
  type UserProfile,
} from "#lib/auth/auth-client.js";
import { clearSignOutNotice } from "#lib/auth/sign-out-notice.svelte.js";
import { readRaw, removeRaw, writeRaw } from "#lib/storage/safe-storage.js";

/**
 * Client-side "this browser has signed in before" marker. Public reader pages
 * mount `Nav`, so an unguarded `hydrate()` fires `GET /api/user/v1/get` on every
 * cold load — a guaranteed 401 for the anonymous majority. The hint gates that
 * probe; it is a UX hint only and never grants access (the session cookie is
 * still the sole authority, and a stale hint self-heals on the next 401).
 */
const SESSION_HINT_KEY = "eq.auth.session-hint";

function hasSessionHint(): boolean {
  return readRaw("local", SESSION_HINT_KEY) !== null;
}

/**
 * Per-login nonce hint value (EC-31): a fixed `"1"` never changes between
 * accounts, so an account switch rewrites `"1"` → `"1"` and fires no `storage`
 * event in other tabs. Every sign-in mints a fresh value instead — never the
 * user id, which would leak an account identifier into shared storage.
 */
function newSessionHintNonce(): string {
  const uuid = globalThis.crypto?.randomUUID?.();
  if (uuid) return uuid;
  return `hint-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export type AuthStatus = "unknown" | "anonymous" | "authenticated";

export type AuthTransitionKind =
  | "login"
  | "logout"
  | "oauth"
  | "passkey"
  | "session-rotated"
  | "two-fa-verify"
  | "two-fa-disable"
  | "current-session-terminated";

export interface AuthTransitionContext {
  readonly kind: AuthTransitionKind;
}

export type AuthTransitionHook = (ctx: AuthTransitionContext) => Promise<void> | void;

/**
 * How often a signed-in tab re-probes the session while idle (EC-15/16/19).
 * Without this, a session revoked elsewhere (terminate from /account, expiry,
 * ban) is invisible until the tab happens to make its next guarded request —
 * an idle reader tab never does, so the expiry banner never appears. One
 * `GET /user/v1/get` per interval is the whole cost; the probe result runs the
 * normal `#applyProbe` mapping (anonymous from authenticated → banner +
 * `current-session-terminated`).
 */
export const SESSION_PROBE_INTERVAL_MS = 60_000;

class AuthState {
  #status = $state<AuthStatus>("unknown");
  #user = $state<UserProfile | null>(null);
  #twoFaPending = $state(false);
  #sessionExpired = $state(false);
  #hydrated = false;
  #hintListener: ((event: StorageEvent) => void) | null = null;
  #probeInFlight: Promise<SessionProbeResult> | null = null;
  #probeTimer: ReturnType<typeof setInterval> | null = null;
  #onAuthTransition: AuthTransitionHook | null = null;
  readonly #client: AuthClient;

  constructor(client: AuthClient = authClient) {
    this.#client = client;
  }

  get status(): AuthStatus {
    return this.#status;
  }

  get user(): UserProfile | null {
    return this.#user;
  }

  get authenticated(): boolean {
    return this.#status === "authenticated";
  }

  get anonymous(): boolean {
    return this.#status === "anonymous";
  }

  get unknown(): boolean {
    return this.#status === "unknown";
  }

  get twoFaPending(): boolean {
    return this.#twoFaPending;
  }

  /**
   * True after a probe-confirmed mid-use session loss (expiry, remote
   * terminate, ban — EC-15/16/19). Drives the "Session expired — sign in to
   * resume sync" banner until the next successful sign-in.
   */
  get sessionExpired(): boolean {
    return this.#sessionExpired;
  }

  get hydrated(): boolean {
    return this.#hydrated;
  }

  setOnAuthTransition(hook: AuthTransitionHook | null): void {
    this.#onAuthTransition = hook;
  }

  /**
   * Idle-tab revocation watchdog: while the tab is signed in, re-probe on a
   * slow interval so a remotely terminated/expired session still reaches the
   * `#terminate(true)` banner path (EC-15). Single interval, browser only;
   * `probe()`'s in-flight dedupe absorbs an overlap with a caller-driven probe.
   */
  #startSessionProbeTimer(): void {
    if (!browser || this.#probeTimer !== null) return;
    this.#probeTimer = setInterval(() => {
      if (this.#status !== "authenticated") return;
      void this.probe();
    }, SESSION_PROBE_INTERVAL_MS);
  }

  #stopSessionProbeTimer(): void {
    if (this.#probeTimer === null) return;
    clearInterval(this.#probeTimer);
    this.#probeTimer = null;
  }

  probe(): Promise<SessionProbeResult> {
    if (this.#probeInFlight) return this.#probeInFlight;
    this.#probeInFlight = (async () => {
      try {
        const result = await this.#client.getUser();
        // Await so transition hooks (purge, account-view reset) complete before
        // the probe resolves — callers act on settled state.
        await this.#applyProbe(result);
        return result;
      } finally {
        this.#probeInFlight = null;
      }
    })();
    return this.#probeInFlight;
  }

  /**
   * `force` skips the session-hint gate: auth/account routes must resolve the
   * real session even when the hint is missing (cleared storage, a session
   * established outside this browser's storage, a first load after this landed).
   */
  hydrate(opts: { force?: boolean } = {}): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#installHintListener();
    if (!opts.force && !hasSessionHint()) {
      this.#status = "anonymous";
      return;
    }
    void this.probe();
  }

  setUser(user: UserProfile | null): void {
    this.#user = user;
    this.#status = user ? "authenticated" : "anonymous";
    if (user) {
      this.#sessionExpired = false;
      // A signed-in edge invalidates any unread sign-out notice (§9.5).
      clearSignOutNotice();
      // Fresh nonce on every sign-in so other tabs see a changed value (EC-31).
      writeRaw("local", SESSION_HINT_KEY, newSessionHintNonce());
      this.#startSessionProbeTimer();
    } else {
      this.#stopSessionProbeTimer();
      removeRaw("local", SESSION_HINT_KEY);
    }
  }

  setTwoFaPending(pending: boolean): void {
    this.#twoFaPending = pending;
  }

  setUnknown(): void {
    this.#stopSessionProbeTimer();
    this.#status = "unknown";
    this.#user = null;
    this.#twoFaPending = false;
  }

  async transition(ctx: AuthTransitionContext): Promise<void> {
    if (this.#onAuthTransition) await this.#onAuthTransition(ctx);
  }

  /**
   * Producer for guarded-route 401/403 rejection (e.g. the bookmarks domain's
   * account-mismatch path): drop the possibly-stale CSRF token and let a probe
   * decide — an anonymous probe from a previously authenticated view runs the
   * `current-session-terminated` transition (EC-15/16/19), a transport failure
   * changes nothing.
   */
  notifySessionLost(): void {
    if (this.#status !== "authenticated") return;
    this.#client.clearCsrf();
    void this.probe();
  }

  reset(): void {
    this.#stopSessionProbeTimer();
    this.#status = "unknown";
    this.#user = null;
    this.#twoFaPending = false;
    this.#sessionExpired = false;
    this.#probeInFlight = null;
    this.#hydrated = false;
    removeRaw("local", SESSION_HINT_KEY);
  }

  #installHintListener(): void {
    // App-lifetime singleton: the listener stays for the tab's life (dispose()
    // exists for test isolation). Storage events only fire in *other* tabs, so
    // this is the cross-tab signal (EC-31).
    if (this.#hintListener !== null) return;
    const onHint = (event: StorageEvent): void => {
      if (event.key !== SESSION_HINT_KEY) return;
      if (event.newValue === null) {
        // Sign-out (or expiry clean-up) in another tab: transition this tab to
        // signed-out without a banner — the user asked for it over there. The
        // account's outbox partition is untouched (quarantine, §9.5).
        void this.#terminate(false);
        return;
      }
      // Sign-in / account switch elsewhere: re-probe and adopt whatever the
      // cookie now says (#applyProbe swaps users without duplicating).
      void this.probe();
    };
    this.#hintListener = onHint;
    window.addEventListener("storage", onHint);
  }

  /** Drop the cross-tab listener (and the revocation watchdog). Test isolation only — the app singleton lives for the tab. */
  dispose(): void {
    this.#stopSessionProbeTimer();
    if (this.#hintListener === null) return;
    window.removeEventListener("storage", this.#hintListener);
    this.#hintListener = null;
  }

  async #terminate(banner: boolean): Promise<void> {
    if (this.#status !== "authenticated") return;
    this.#stopSessionProbeTimer();
    this.#client.clearCsrf();
    if (banner) this.#sessionExpired = true;
    this.#status = "anonymous";
    this.#user = null;
    this.#twoFaPending = false;
    removeRaw("local", SESSION_HINT_KEY);
    await this.transition({ kind: "current-session-terminated" });
  }

  async #applyProbe(result: SessionProbeResult): Promise<void> {
    if (result.kind === "authenticated") {
      const previous = this.#user;
      // Cross-tab adoption of a different account (EC-31): run the login
      // transition so account views reset, exactly like the in-tab adopt path.
      // Cold boots (previous === null) must NOT purge on every load.
      if (previous !== null && previous.id !== result.user.id) {
        await this.transition({ kind: "login" });
      }
      this.#status = "authenticated";
      this.#user = result.user;
      this.#twoFaPending = false;
      this.#sessionExpired = false;
      this.#startSessionProbeTimer();
      // Cross-tab sign-in adoption invalidates an unread sign-out notice too.
      clearSignOutNotice();
      // Present-but-stale hint self-heals through the probe; only mint a nonce
      // when absent so re-probes never churn the value other tabs watch.
      if (readRaw("local", SESSION_HINT_KEY) === null) {
        writeRaw("local", SESSION_HINT_KEY, newSessionHintNonce());
      }
    } else if (result.kind === "anonymous") {
      if (this.#status === "authenticated") {
        // Probe-confirmed mid-use loss: expiry, remote terminate, or ban
        // (EC-15/16/19) — banner + terminate transition.
        await this.#terminate(true);
        return;
      }
      this.#stopSessionProbeTimer();
      this.#status = "anonymous";
      this.#user = null;
      this.#twoFaPending = false;
      removeRaw("local", SESSION_HINT_KEY);
    }
  }
}

export function createAuthState(client: AuthClient = authClient): AuthState {
  return new AuthState(client);
}

export const authState: AuthState = createAuthState();
