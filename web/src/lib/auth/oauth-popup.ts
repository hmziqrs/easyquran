import { browser } from "$app/env";

/**
 * Channel between the OAuth popup's landing pages (success/failure routes) and
 * the tab that opened it. The popup carries no tokens or profile data — the
 * backend callback owns the session; the message is a one-shot completion
 * signal that lets the opener run its normal finish() probe and navigation.
 */
export const OAUTH_POPUP_SOURCE = "eq:oauth-popup";

/** window.open name prefix; the landing pages use it to recognize the popup. */
export const OAUTH_POPUP_NAME_PREFIX = "eq-oauth-";

export type OAuthPopupOutcome =
  | "completed"
  | "failed"
  | "cancelled"
  | "blocked"
  | "unavailable"
  | "in_flight";

export interface OAuthPopupMessage {
  readonly source: typeof OAUTH_POPUP_SOURCE;
  readonly provider: string;
  readonly outcome: "success" | "failure";
}

interface OAuthPopupWire {
  readonly source?: string;
  readonly provider?: string;
  readonly outcome?: string;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- data is the raw postMessage payload; this predicate is the boundary parser that validates it
export function isOAuthPopupMessage(data: unknown): data is OAuthPopupMessage {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- postMessage payload is unparsed unknown at the boundary; the field checks below are the parser
  if (typeof data !== "object" || data === null) return false;
  // SAFETY: the typeof-object check above is the only way to read fields off an unknown postMessage payload before narrowing.
  const o = data as OAuthPopupWire;
  const outcomeOk = o.outcome === "success" || o.outcome === "failure";
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- provider is a wire string discriminator with no static type at this boundary
  return o.source === OAUTH_POPUP_SOURCE && typeof o.provider === "string" && outcomeOk;
}

let popupNameSeq = 0;

/** Unique per attempt: a fixed name lets a later attempt reuse an earlier popup window. */
export function oauthPopupName(provider: string): string {
  popupNameSeq += 1;
  return `${OAUTH_POPUP_NAME_PREFIX}${provider}-${Date.now().toString(36)}-${popupNameSeq}`;
}

function isOAuthPopupWindow(): boolean {
  // Some browsers clear window.name across origins, so only enforce the popup
  // prefix when a name survived the roundtrip.
  return window.name.length === 0 || window.name.startsWith(OAUTH_POPUP_NAME_PREFIX);
}

/**
 * Runs inside the popup's landing page. Returns true only when this window is
 * the popup our app opened (same-origin opener, plus the eq-oauth name when the
 * browser kept one); the caller then skips normal completion (the opener owns
 * the finish() probe and destination navigation) and the popup closes itself.
 * Any other window — an app tab opened from a foreign site, a same-origin tab
 * that is not our popup — returns false and completes normally.
 */
export function notifyOAuthPopupOpener(provider: string, outcome: "success" | "failure"): boolean {
  if (!browser) return false;
  if (!isOAuthPopupWindow()) return false;
  const opener: Window | null = window.opener;
  if (!opener) return false;
  let sameOrigin = false;
  try {
    sameOrigin = opener.location.origin === window.location.origin;
  } catch {
    // Reading a cross-origin opener's location throws; it is not our popup host.
    return false;
  }
  if (!sameOrigin) return false;
  const message: OAuthPopupMessage = { source: OAUTH_POPUP_SOURCE, provider, outcome };
  // Same-origin target: a foreign opener never receives app state, and this page
  // never trusts the opener beyond delivering the one-shot signal.
  opener.postMessage(message, window.location.origin);
  window.close();
  return true;
}

export interface OAuthPopupHandle {
  readonly closed: boolean;
  close(): void;
  focus?(): void;
  /** Real popups expose location; injected handles may provide navigate() instead. */
  readonly location?: { href: string };
  navigate?(url: string): void;
}

/** Points an already-open popup at the authorize URL (it was opened about:blank). */
export function navigateOAuthPopup(popup: OAuthPopupHandle, url: string): void {
  if (popup.navigate) {
    popup.navigate(url);
    return;
  }
  if (popup.location) popup.location.href = url;
}

export type OAuthPopupOpen = (
  url: string,
  name: string,
  features: string,
) => OAuthPopupHandle | null;

export function defaultPopupOpen(
  url: string,
  name: string,
  features: string,
): OAuthPopupHandle | null {
  if (!browser) return null;
  return window.open(url, name, features);
}

const POPUP_WIDTH = 480;
const POPUP_HEIGHT = 640;
const POPUP_POLL_MS = 400;
const POPUP_CLOSE_GRACE_MS = 250;
const POPUP_DEADLINE_MS = 5 * 60 * 1000;

/** Centered popup features; degrades to defaults where the browser reports no geometry. */
export function oauthPopupFeatures(): string {
  if (!browser) return "";
  const outerWidth = Number.isFinite(window.outerWidth) ? window.outerWidth : POPUP_WIDTH;
  const outerHeight = Number.isFinite(window.outerHeight) ? window.outerHeight : POPUP_HEIGHT;
  const screenX = Number.isFinite(window.screenX) ? window.screenX : 0;
  const screenY = Number.isFinite(window.screenY) ? window.screenY : 0;
  const left = Math.max(0, Math.round(screenX + (outerWidth - POPUP_WIDTH) / 2));
  const top = Math.max(0, Math.round(screenY + (outerHeight - POPUP_HEIGHT) / 2));
  return `popup=yes,width=${POPUP_WIDTH},height=${POPUP_HEIGHT},left=${left},top=${top}`;
}

export interface AwaitOAuthPopupOptions {
  readonly provider: string;
  readonly url: string;
  readonly open?: OAuthPopupOpen;
  /** Already-open popup (opened synchronously from the click gesture). */
  readonly popup?: OAuthPopupHandle;
  readonly features?: string;
  readonly pollMs?: number;
  readonly closeGraceMs?: number;
  readonly deadlineMs?: number;
}

/**
 * Resolves once the landing page reports success/failure, the popup closes
 * first (cancelled), it never opens (blocked), or the overall deadline passes
 * (unavailable). Never rejects — callers fall back to the full-page redirect on
 * every outcome except "completed". Every settle path removes the listener and
 * clears the poll/grace/deadline timers.
 */
export function awaitOAuthPopup(options: AwaitOAuthPopupOptions): Promise<OAuthPopupOutcome> {
  const popup =
    options.popup ??
    (options.open ?? defaultPopupOpen)(
      options.url,
      oauthPopupName(options.provider),
      options.features ?? oauthPopupFeatures(),
    );
  if (!popup) return Promise.resolve("blocked");

  return new Promise<OAuthPopupOutcome>((resolve) => {
    const pollMs = options.pollMs ?? POPUP_POLL_MS;
    const closeGraceMs = options.closeGraceMs ?? POPUP_CLOSE_GRACE_MS;
    const deadlineMs = options.deadlineMs ?? POPUP_DEADLINE_MS;
    let settled = false;
    let closeTimer: ReturnType<typeof setTimeout> | null = null;

    const settle = (outcome: OAuthPopupOutcome): void => {
      if (settled) return;
      settled = true;
      window.removeEventListener("message", onMessage);
      clearInterval(pollTimer);
      if (closeTimer !== null) clearTimeout(closeTimer);
      clearTimeout(deadlineTimer);
      resolve(outcome);
    };

    const onMessage = (event: MessageEvent): void => {
      if (event.origin !== window.location.origin) return;
      // Only the exact window this attempt opened may complete it.
      if (event.source !== popup) return;
      const data: unknown = event.data;
      if (!isOAuthPopupMessage(data)) return;
      if (data.provider !== options.provider) return;
      settle(data.outcome === "success" ? "completed" : "failed");
    };

    const pollTimer = setInterval(() => {
      if (settled || !popup.closed) return;
      // A completion message may still be in flight when the popup closes; the
      // grace window lets it land before reporting a cancel.
      closeTimer ??= setTimeout(() => settle("cancelled"), closeGraceMs);
    }, pollMs);

    // A popup that neither messages nor closes must not leave the attempt (and
    // its listener/timers) hanging forever; the caller falls back after this.
    const deadlineTimer = setTimeout(() => {
      popup.close();
      settle("unavailable");
    }, deadlineMs);

    window.addEventListener("message", onMessage);
    popup.focus?.();
  });
}
