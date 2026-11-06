import {
  identityProviderLabel,
  isIdentityProvider,
  type DeviceInfo,
  type IdentityProvider,
} from "#lib/auth/account-client.js";
import type { AuthStatus } from "#lib/auth/auth-state.svelte.js";
import type { AuthCopy } from "#lib/i18n/auth-copy.js";

export type AccountView = "loading" | "anonymous" | "authenticated";

export const ACCOUNT_PATH = "/account";
export const ANONYMOUS_CTA_HREF = "/login";

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

// `unknownLabel` is the localized fallback; the table labels stay English brand names.
export function sessionDeviceLabel(userAgent?: string, unknownLabel = "Unknown device"): string {
  if (!userAgent) return unknownLabel;
  const browser = firstMatch(userAgent, BROWSERS);
  const os = firstMatch(userAgent, OPERATING_SYSTEMS);
  if (browser !== null && os !== null) return `${browser} · ${os}`;
  if (browser !== null) return browser;
  if (os !== null) return os;
  return unknownLabel;
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

// Device labels (devices.md D4). Like the session labels above, every name comes from
// a fixed table keyed by the closed server enums, so an unknown value falls back to
// the localized unknown label and nothing from the wire is echoed as a label.
const BROWSER_FAMILIES = new Map([
  ["chrome", "Chrome"],
  ["firefox", "Firefox"],
  ["safari", "Safari"],
  ["edge", "Edge"],
]);

const PLATFORM_LABELS = new Map([
  ["macos", "Mac"],
  ["windows", "Windows PC"],
  ["linux", "Linux PC"],
]);

function webBrowserLabel(device: DeviceInfo, unknownLabel: string): string {
  if (device.browser === null) return unknownLabel;
  return BROWSER_FAMILIES.get(device.browser) ?? unknownLabel;
}

function mobileLabel(device: DeviceInfo): string | null {
  const tablet = device.formFactor === "tablet";
  if (device.platform === "ios") return tablet ? "iPad" : "iPhone";
  if (device.platform === "android") return tablet ? "Android tablet" : "Android phone";
  return null;
}

/** Browser for the web client, platform otherwise. The unnamed fallback for a device. */
export function deviceTypeLabel(device: DeviceInfo, unknownLabel = "Unknown device"): string {
  if (device.platform === "web") return webBrowserLabel(device, unknownLabel);
  return mobileLabel(device) ?? PLATFORM_LABELS.get(device.platform) ?? unknownLabel;
}

/** The row title: the user's own name when set, else the device type. */
export function deviceTitle(device: DeviceInfo, unknownLabel = "Unknown device"): string {
  if (device.displayName !== null) return device.displayName;
  return deviceTypeLabel(device, unknownLabel);
}

/** Phone-shaped for iOS and Android, monitor-shaped for everything else. */
export function deviceIconName(device: DeviceInfo): "device-phone" | "device-monitor" {
  const handheld = device.platform === "ios" || device.platform === "android";
  return handheld ? "device-phone" : "device-monitor";
}

/** Names the server accepts: 1 to 40 characters after trimming. Null when out of range. */
export const DEVICE_NAME_MAX_CHARS = 40;

export function normaliseDeviceNameDraft(draft: string): string | null {
  const name = draft.trim();
  const length = Array.from(name).length;
  if (length === 0 || length > DEVICE_NAME_MAX_CHARS) return null;
  return name;
}
