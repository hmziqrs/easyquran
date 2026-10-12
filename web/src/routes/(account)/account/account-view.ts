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

// Labels come only from these fixed tables: a session row's device string is
// attacker-controlled input (server-stored UA), so the label must never echo
// it. Order matters — Chromium forks are matched before the generic Chrome and
// Safari tokens their UAs also carry.
const BROWSERS: ReadonlyArray<readonly [RegExp, string]> = [
  [/edg(?:e|ios|a)?\//iu, "Edge"],
  [/opr\/|opios\//iu, "Opera"],
  [/samsungbrowser\//iu, "Samsung Internet"],
  [/firefox\/|fxios\//iu, "Firefox"],
  [/chrome\/|crios\//iu, "Chrome"],
  [/safari\//iu, "Safari"],
];

const OPERATING_SYSTEMS: ReadonlyArray<readonly [RegExp, string]> = [
  [/windows/iu, "Windows"],
  [/cros/iu, "ChromeOS"],
  // iOS/iPadOS UAs end in "like Mac OS X", so they must match before macOS.
  [/iphone|ipod/iu, "iOS"],
  [/ipad/iu, "iPadOS"],
  [/android/iu, "Android"],
  [/mac os x|macintosh/iu, "macOS"],
  [/linux/iu, "Linux"],
];

function firstMatch(
  source: string,
  patterns: ReadonlyArray<readonly [RegExp, string]>,
): string | null {
  for (const [pattern, label] of patterns) {
    if (pattern.test(source)) return label;
  }
  return null;
}

export function sessionDeviceLabel(userAgent?: string): string {
  if (!userAgent) return "Unknown device";
  const browser = firstMatch(userAgent, BROWSERS);
  const os = firstMatch(userAgent, OPERATING_SYSTEMS);
  if (browser !== null && os !== null) return `${browser} · ${os}`;
  if (browser !== null) return browser;
  if (os !== null) return os;
  return "Unknown device";
}

// Locale-aware "Oct 12, 07:47" for a session's last_seen stamp. Returns null
// for missing or unparsable stamps so the row can hide the line instead of
// rendering raw ISO text.
export function sessionLastSeenLabel(iso: string | undefined, locale: string): string | null {
  if (!iso) return null;
  // The server sends sub-millisecond precision (e.g. …12.919710Z); trim to
  // milliseconds so engines that reject >3 fraction digits still parse it.
  const trimmed = iso.replace(/(\.\d{3})\d+/u, "$1");
  const parsed = Date.parse(trimmed);
  if (Number.isNaN(parsed)) return null;
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(parsed);
}
