import { browser } from "$app/env";

/**
 * Channel between the OAuth popup's landing pages (success/failure routes) and
 * the tab that opened it. The popup carries no tokens or profile data — the
 * backend callback owns the session; the message is a one-shot completion
 * signal that lets the opener run its normal finish() probe and navigation.
 */
export const OAUTH_POPUP_SOURCE = "eq:oauth-popup";

export type OAuthPopupOutcome = "completed" | "failed" | "cancelled" | "blocked" | "unavailable";

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

/**
 * Runs inside the popup's landing page. Returns true when an opener existed and
 * was notified; the caller then skips normal completion (the opener owns the
 * finish() probe and destination navigation) and the popup closes itself.
 */
export function notifyOAuthPopupOpener(provider: string, outcome: "success" | "failure"): boolean {
  if (!browser) return false;
  const opener: Window | null = window.opener;
  if (!opener) return false;
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
  readonly features?: string;
  readonly pollMs?: number;
  readonly closeGraceMs?: number;
}

/**
 * Opens the provider popup and resolves once the landing page reports
 * success/failure, the popup closes first (cancelled), or it never opens
 * (blocked). Never rejects — callers fall back to the full-page redirect on
 * every outcome except "completed".
 */
export function awaitOAuthPopup(options: AwaitOAuthPopupOptions): Promise<OAuthPopupOutcome> {
  const open = options.open ?? defaultPopupOpen;
  const popup = open(
    options.url,
    `eq-oauth-${options.provider}`,
    options.features ?? oauthPopupFeatures(),
  );
  if (!popup) return Promise.resolve("blocked");

  return new Promise<OAuthPopupOutcome>((resolve) => {
    const pollMs = options.pollMs ?? POPUP_POLL_MS;
    const closeGraceMs = options.closeGraceMs ?? POPUP_CLOSE_GRACE_MS;
    let settled = false;
    let closeTimer: ReturnType<typeof setTimeout> | null = null;

    const settle = (outcome: OAuthPopupOutcome): void => {
      if (settled) return;
      settled = true;
      window.removeEventListener("message", onMessage);
      clearInterval(pollTimer);
      if (closeTimer !== null) clearTimeout(closeTimer);
      resolve(outcome);
    };

    const onMessage = (event: MessageEvent): void => {
      if (event.origin !== window.location.origin) return;
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

    window.addEventListener("message", onMessage);
    popup.focus?.();
  });
}
