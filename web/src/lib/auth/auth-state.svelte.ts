import { browser } from "$app/environment";
import {
  authClient,
  type AuthClient,
  type SessionProbeResult,
  type UserProfile,
} from "$lib/auth/auth-client";

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

class AuthState {
  #status = $state<AuthStatus>("unknown");
  #user = $state<UserProfile | null>(null);
  #twoFaPending = $state(false);
  #hydrated = false;
  #probeInFlight: Promise<SessionProbeResult> | null = null;
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

  get hydrated(): boolean {
    return this.#hydrated;
  }

  setOnAuthTransition(hook: AuthTransitionHook | null): void {
    this.#onAuthTransition = hook;
  }

  probe(): Promise<SessionProbeResult> {
    if (this.#probeInFlight) return this.#probeInFlight;
    this.#probeInFlight = (async () => {
      try {
        const result = await this.#client.getUser();
        this.#applyProbe(result);
        return result;
      } finally {
        this.#probeInFlight = null;
      }
    })();
    return this.#probeInFlight;
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    void this.probe();
  }

  setUser(user: UserProfile | null): void {
    this.#user = user;
    this.#status = user ? "authenticated" : "anonymous";
  }

  setTwoFaPending(pending: boolean): void {
    this.#twoFaPending = pending;
  }

  setUnknown(): void {
    this.#status = "unknown";
    this.#user = null;
    this.#twoFaPending = false;
  }

  async transition(ctx: AuthTransitionContext): Promise<void> {
    if (this.#onAuthTransition) await this.#onAuthTransition(ctx);
  }

  reset(): void {
    this.#status = "unknown";
    this.#user = null;
    this.#twoFaPending = false;
    this.#probeInFlight = null;
    this.#hydrated = false;
  }

  #applyProbe(result: SessionProbeResult): void {
    if (result.kind === "authenticated") {
      this.#status = "authenticated";
      this.#user = result.user;
      this.#twoFaPending = false;
    } else if (result.kind === "anonymous") {
      this.#status = "anonymous";
      this.#user = null;
      this.#twoFaPending = false;
    }
  }
}

export function createAuthState(client: AuthClient = authClient): AuthState {
  return new AuthState(client);
}

export const authState: AuthState = createAuthState();
