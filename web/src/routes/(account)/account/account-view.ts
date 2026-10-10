import {
  identityProviderLabel,
  isIdentityProvider,
  type IdentityProvider,
} from "#lib/auth/account-client.js";
import type { AuthStatus } from "#lib/auth/auth-state.svelte.js";
import type { AuthCopy } from "#lib/i18n/auth-copy.js";

export type AccountView = "loading" | "anonymous" | "authenticated";

export const ACCOUNT_PATH = "/account";
export const ANONYMOUS_CTA_HREF = "/login";
export const REGISTER_HREF = "/register";

export interface LinkNotice {
  readonly kind: "success" | "error";
  readonly text: string;
}

// Minimal read side shared by URLSearchParams and SvelteKit's ReadonlyURLSearchParams.
export interface LinkNoticeParams {
  get(name: string): string | null;
}

// Maps the link-flow redirect params to banner copy. `link_error` carries no
// provider, so the already-linked message names the provider stashed by the
// component before it left for the provider (link-attempt.ts); without one it
// falls back to the generic failure copy.
export function resolveLinkNotice(
  params: LinkNoticeParams,
  copy: AuthCopy,
  attemptedProvider: IdentityProvider | null,
): LinkNotice | null {
  const linked = params.get("linked");
  if (linked !== null && isIdentityProvider(linked)) {
    return { kind: "success", text: copy.connectedLinkedBanner(identityProviderLabel(linked)) };
  }
  const error = params.get("link_error");
  if (error === "cancelled") return { kind: "error", text: copy.connectedCancelledError };
  if (error === "link_failed") return { kind: "error", text: copy.connectedFailedError };
  if (error === "already_linked") {
    if (attemptedProvider === null) return { kind: "error", text: copy.connectedFailedError };
    return {
      kind: "error",
      text: copy.connectedAlreadyLinkedError(identityProviderLabel(attemptedProvider)),
    };
  }
  return null;
}

export function resolveAccountView(status: AuthStatus): AccountView {
  if (status === "authenticated") return "authenticated";
  if (status === "anonymous") return "anonymous";
  return "loading";
}

export function sessionDeviceLabel(userAgent?: string): string {
  if (!userAgent) return "This device";
  if (/mobile|android|iphone|ipod/i.test(userAgent)) return "Mobile device";
  if (/ipad|tablet/i.test(userAgent)) return "Tablet";
  return "This device";
}
