/* ════════════════════════════════════════════════════════════════════════
   firebase/messaging.ts — Firebase Cloud Messaging (web push).

   A thin, browser-only client. Browser APIs + the SDK are loaded LAZILY; every
   entry point is guarded and swallows errors so a push failure can never break
   reading. The stateful lifecycle (when to ask permission, when to register the
   token with the backend, what to show the user) lives in stores/notifications;
   this module exposes the primitives.

   Background delivery (page closed / backgrounded) is handled by the service
   worker at src/service-worker.ts, which imports firebase/messaging/sw.
   Foreground delivery (page focused) is surfaced two ways:
     • onForegroundMessage(cb)            — programmatic listener (Unsubscribe)
     • window event "easyquran:fcm"        — { detail: MessagePayload }

   Token refresh: the modular messaging SDK exposes no refresh callback. The
   recommended pattern is to re-call getToken() (it returns the current, possibly
   refreshed token) on app focus; the notifications store does this on
   visibilitychange and compares against the stored token.

   The VAPID key (PUBLIC_FIREBASE_VAPID_KEY) is required; without it getToken
   cannot run. Backend registration is optional: only when PUBLIC_API_BASE_URL
   is set does the token get POSTed to the Axum `/device/v1/*` routes (which
   require an authenticated session — until accounts ship, the token is kept
   locally and re-registered on a future login).
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { Messaging, MessagePayload, Unsubscribe } from "firebase/messaging";
import { isConfigured, FCM_VAPID_KEY, API_BASE_URL, initApp } from "./index";

export type PermissionState = "granted" | "denied" | "default" | "unsupported";

let messaging: Messaging | null = null;
let messagingPromise: Promise<Messaging | null> | null = null;

const foregroundCallbacks = new Set<(payload: MessagePayload) => void>();

/** Cheap, synchronous permission read from the Notifications API. */
export function getPermissionState(): PermissionState {
  if (!browser || !("Notification" in window)) return "unsupported";
  return Notification.permission as PermissionState;
}

/** Async SDK-level support check (touches IndexedDB; also feature-detects first). */
export async function isMessagingSupported(): Promise<boolean> {
  if (!browser || !isConfigured || !FCM_VAPID_KEY) return false;
  if (
    !("serviceWorker" in navigator) ||
    !("PushManager" in window) ||
    !("Notification" in window)
  ) {
    return false;
  }
  try {
    const { isSupported } = await import("firebase/messaging");
    return await isSupported();
  } catch {
    return false;
  }
}

/**
 * Initialize Messaging exactly once and wire the foreground + token-refresh
 * listeners that broadcast to the page. Browser-only; no-op if unconfigured.
 */
export function initMessaging(): Promise<Messaging | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!messagingPromise) {
    messagingPromise = (async () => {
      try {
        const core = await initApp();
        if (!core) return null;
        const { getMessaging, onMessage } = await import("firebase/messaging");
        const m = getMessaging(core);
        onMessage(m, (payload) => {
          window.dispatchEvent(new CustomEvent("easyquran:fcm", { detail: payload }));
          for (const cb of foregroundCallbacks) {
            try {
              cb(payload);
            } catch (err) {
              console.warn("[firebase] onMessage listener threw:", err);
            }
          }
        });
        messaging = m;
      } catch (err) {
        console.warn("[firebase] messaging failed to initialize:", err);
      }
      return messaging;
    })();
  }
  return messagingPromise;
}

/**
 * Register the root service worker (static/sw.js at /) — the ONE worker that
 * both caches the app shell AND handles Firebase Cloud Messaging background
 * push — and resolve once it's active. Idempotent. Returns null if service
 * workers are unavailable or registration throws. (The worker is also registered
 * eagerly by the root layout on first paint; this re-register is a no-op that
 * just yields the existing registration for getToken.)
 */
export async function ensureServiceWorker(): Promise<ServiceWorkerRegistration | null> {
  if (!browser || !("serviceWorker" in navigator)) return null;
  try {
    const reg = await navigator.serviceWorker.register("/sw.js", { scope: "/" });
    await navigator.serviceWorker.ready;
    return reg;
  } catch (err) {
    console.warn("[firebase] service worker registration failed:", err);
    return null;
  }
}

/** Prompt for notification permission (call from a user gesture). */
export async function requestPermission(): Promise<PermissionState> {
  if (!browser || !("Notification" in window)) return "unsupported";
  try {
    const result = await Notification.requestPermission();
    return result as PermissionState;
  } catch {
    return getPermissionState();
  }
}

/**
 * Obtain the FCM registration token. Requires granted permission, a VAPID key,
 * and an active service worker. Returns null on any failure (incl. permission
 * denied / blocked / unsupported) — callers treat null as "not subscribed".
 */
export async function getFcmToken(): Promise<string | null> {
  if (!isConfigured || !FCM_VAPID_KEY) {
    console.warn("[firebase] messaging: VAPID key missing (PUBLIC_FIREBASE_VAPID_KEY).");
    return null;
  }
  if (getPermissionState() !== "granted") return null;
  const m = await initMessaging();
  if (!m) return null;
  const reg = await ensureServiceWorker();
  if (!reg) return null;
  try {
    const { getToken } = await import("firebase/messaging");
    return await getToken(m, {
      vapidKey: FCM_VAPID_KEY,
      serviceWorkerRegistration: reg,
    });
  } catch (err) {
    console.warn("[firebase] getToken failed:", err);
    return null;
  }
}

/** Invalidate the current token on this device (FCM-side). */
export async function deleteFcmToken(): Promise<void> {
  const m = messaging ?? (await initMessaging());
  if (!m) return;
  try {
    const { deleteToken } = await import("firebase/messaging");
    await deleteToken(m);
  } catch (err) {
    console.warn("[firebase] deleteToken failed:", err);
  }
}

/** Register a foreground-message listener; returns an unsubscribe function. */
export async function onForegroundMessage(
  cb: (payload: MessagePayload) => void,
): Promise<Unsubscribe> {
  foregroundCallbacks.add(cb);
  await initMessaging();
  return () => {
    foregroundCallbacks.delete(cb);
  };
}

/* ── Optional backend registration (Axum `/device/v1/*`) ────────────────── */

/**
 * POST the token to the backend so the server can target this device. Returns
 * false when there's no API base configured or the request fails; callers keep
 * the token locally and retry on a future login.
 *
 * NOTE(accounts): the Axum `/device/v1/register` route is behind `auth_guard`
 * (logged-in, verified user) AND the global `csrf_guard` (double-submit). Until
 * accounts ship, this returns false (401/403) and the token stays local — by
 * design. When the web API client + auth land, send `credentials: "include"`
 * (already set) plus a `csrf-token` header obtained from `/csrf/v1/generate`.
 */
export async function registerTokenWithServer(token: string): Promise<boolean> {
  if (!API_BASE_URL) return false;
  try {
    const res = await fetch(`${API_BASE_URL}/device/v1/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ token, platform: "web" }),
    });
    return res.ok;
  } catch (err) {
    console.warn("[firebase] device register failed:", err);
    return false;
  }
}

/**
 * DELETE the token from the backend. Same auth/CSRF requirements as register;
 * no-op when no API base is configured (see registerTokenWithServer).
 */
export async function unregisterTokenFromServer(token: string): Promise<boolean> {
  if (!API_BASE_URL) return false;
  try {
    const res = await fetch(`${API_BASE_URL}/device/v1/delete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ token }),
    });
    return res.ok;
  } catch {
    return false;
  }
}
