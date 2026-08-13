import { browser } from "$app/environment";
import {
  authClient,
  decodeUserProfile,
  type AuthClient,
  type UserProfile,
} from "$lib/auth/auth-client";
import type { SessionProbeResult } from "$lib/auth/auth-client";
import { authState } from "$lib/auth/auth-state.svelte";
import type { AuthTransitionContext } from "$lib/auth/auth-state.svelte";
import { consumeReturnTarget, setReturnTarget } from "$lib/auth/return-target";

export type OAuthProvider = "google" | "apple" | "facebook" | "github";

const PROVIDERS: ReadonlySet<OAuthProvider> = new Set(["google", "apple", "facebook", "github"]);

export function isOAuthProvider(v: unknown): v is OAuthProvider {
  return typeof v === "string" && PROVIDERS.has(v as OAuthProvider);
}

export interface OAuthFinishResult {
  readonly ok: boolean;
  readonly returnTarget: string | null;
  readonly errorCode?: string;
}

export type OAuthNavigator = (url: string) => void;

export interface OAuthFlowStateLike {
  transition(ctx: AuthTransitionContext): Promise<void>;
  setUser(user: UserProfile | null): void;
  probe(): Promise<SessionProbeResult>;
}

export interface OAuthFlowDeps {
  readonly client?: AuthClient;
  readonly state?: OAuthFlowStateLike;
  readonly navigate?: OAuthNavigator;
}

export const OAUTH_START_FAILED = "oauth_start_failed";
export const OAUTH_FINISH_FAILED = "oauth_failed";

function defaultNavigate(url: string): void {
  if (browser) window.location.assign(url);
}

export class OAuthFlow {
  readonly client: AuthClient;
  readonly state: OAuthFlowStateLike;
  readonly provider: OAuthProvider;
  readonly #navigate: OAuthNavigator;
  pending = $state(false);
  lastErrorCode = $state<string | null>(null);

  constructor(
    provider: OAuthProvider,
    client: AuthClient,
    state: OAuthFlowStateLike,
    navigate: OAuthNavigator = defaultNavigate,
  ) {
    if (!isOAuthProvider(provider)) {
      throw new Error(`unsupported oauth provider: ${String(provider)}`);
    }
    this.provider = provider;
    this.client = client;
    this.state = state;
    this.#navigate = navigate;
  }

  get loginUrl(): string {
    return `${this.client.apiBase}/auth/${this.provider}/v1/login`;
  }

  get exchangeUrl(): string {
    return `${this.client.apiBase}/auth/${this.provider}/v1/exchange`;
  }

  async begin(returnTarget?: string): Promise<boolean> {
    if (this.pending) return false;
    if (!browser) return false;
    this.pending = true;
    this.lastErrorCode = null;
    try {
      await this.client.ensureAnonymousSession();
      if (returnTarget) setReturnTarget(returnTarget);
      this.#navigate(this.loginUrl);
      return true;
    } catch {
      this.lastErrorCode = OAUTH_START_FAILED;
      return false;
    } finally {
      this.pending = false;
    }
  }

  async finish(): Promise<OAuthFinishResult> {
    if (this.pending) {
      return { ok: false, returnTarget: null, errorCode: "in_flight" };
    }
    this.pending = true;
    this.lastErrorCode = null;
    try {
      let user: UserProfile | null = null;
      let rotated = false;
      try {
        const ex = await this.client.unsafeRequest<UserProfile>(
          `/auth/${this.provider}/v1/exchange`,
          { method: "POST" },
        );
        if (ex.ok) {
          user = decodeUserProfile(ex.data);
          rotated = ex.rotated;
        }
      } catch {
        // exchange unavailable (callback may have already established session)
      }
      if (!rotated) {
        try {
          await this.client.refreshCsrf();
        } catch {}
      }
      await this.state.transition({ kind: "oauth" });
      const probe = await this.state.probe();
      if (!user && probe.kind === "authenticated") user = probe.user;
      const target = consumeReturnTarget();
      if (user) {
        this.state.setUser(user);
        return { ok: true, returnTarget: target };
      }
      this.lastErrorCode = OAUTH_FINISH_FAILED;
      return { ok: false, returnTarget: target, errorCode: OAUTH_FINISH_FAILED };
    } catch {
      const target = consumeReturnTarget();
      this.lastErrorCode = OAUTH_FINISH_FAILED;
      return { ok: false, returnTarget: target, errorCode: OAUTH_FINISH_FAILED };
    } finally {
      this.pending = false;
    }
  }

  reportFailure(errorCode: string = OAUTH_FINISH_FAILED): OAuthFinishResult {
    this.lastErrorCode = errorCode;
    const target = consumeReturnTarget();
    return { ok: false, returnTarget: target, errorCode };
  }

  reset(): void {
    this.pending = false;
    this.lastErrorCode = null;
  }
}

export function createOAuthFlow(provider: OAuthProvider, deps: OAuthFlowDeps = {}): OAuthFlow {
  return new OAuthFlow(provider, deps.client ?? authClient, deps.state ?? authState, deps.navigate);
}
