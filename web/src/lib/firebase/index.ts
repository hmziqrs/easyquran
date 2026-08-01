/* ════════════════════════════════════════════════════════════════════════
   firebase/index.ts — shared core: web config + the single FirebaseApp.

   Everything browser-only here is loaded LAZILY via dynamic import from inside
   initApp(), so neither firebase/app nor any feature module ships in the page's
   critical modulepreload graph or runs during SSR/prerender. Every entry point
   is guarded and wrapped in try/catch — nothing here can take the page down.

   The web config is *not* a secret — by Firebase's design it ships in the
   client bundle; access is gated by Security Rules / App Check, not by hiding
   it. It's HARDCODED below (single Firebase project) rather than read from env,
   so analytics/performance/messaging are always configured for this build. The
   prerendered /firebase-config.js service-worker endpoint imports this same
   object — one source of truth. (API_BASE_URL — the Axum device-registration
   origin — is the one exception: it stays an env var because it genuinely varies
   per environment.)
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { env } from "$env/dynamic/public";
import type { FirebaseApp, FirebaseOptions } from "firebase/app";

export const firebaseConfig: FirebaseOptions = {
  apiKey: "AIzaSyBTlP0JPVdOxesSY1QtqFujW4OqmaUsoMg",
  authDomain: "easyquran-fyi.firebaseapp.com",
  projectId: "easyquran-fyi",
  storageBucket: "easyquran-fyi.firebasestorage.app",
  messagingSenderId: "679657424226",
  appId: "1:679657424226:web:3bdc96c4f984fb8294983b",
  measurementId: "G-ZYL6HY24W6",
};

/** Always true now that the config is hardcoded; kept so feature modules still
 *  short-circuit uniformly if the project is ever blanked out. */
export const isConfigured = Boolean(firebaseConfig.apiKey && firebaseConfig.appId);

/**
 * Web Push VAPID key (Firebase console → Project settings → Cloud Messaging →
 * Web configuration → "Generate key pair"). Required for FCM web push; until a
 * key is pasted here, getToken() cannot run and push silently no-ops.
 *   TODO(accounts/push): paste the generated public key to enable notifications.
 */
export const FCM_VAPID_KEY = "";

/**
 * Backend API origin for device-token registration (the Axum `/device/v1/*`
 * routes). Trailing slash trimmed. Empty until accounts arrive — without it the
 * client still obtains an FCM token and keeps it locally, so a later login can
 * register it. Optional: if set, the messaging module POSTs the token to
 * `${API_BASE_URL}/device/v1/register` on subscribe. Unlike the Firebase config
 * above, this genuinely varies per environment (local empty, prod origin), so
 * it stays an env var.
 */
export const API_BASE_URL = (env.PUBLIC_API_BASE_URL || "").replace(/\/+$/, "");

/** When true, analytics calls are echoed to the console (local dev aid). For
 *  GA4 DebugView itself, append `?firebase_debug_mode=1` to the URL instead. */
export const ANALYTICS_DEBUG = false;

let app: FirebaseApp | null = null;
let appPromise: Promise<FirebaseApp | null> | null = null;

export function getFirebaseApp(): FirebaseApp | null {
  return app;
}

export function initApp(): Promise<FirebaseApp | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!appPromise) {
    appPromise = (async () => {
      try {
        const { getApps, getApp, initializeApp } = await import("firebase/app");
        // Reuse an existing app (e.g. one created by another entry) so we never
        // double-initialize, which throws "Firebase App named '[DEFAULT]' already exists".
        const core = getApps().length ? getApp() : initializeApp(firebaseConfig);
        app = core;
      } catch (err) {
        console.warn("[firebase] app failed to initialize:", err);
      }
      return app;
    })();
  }
  return appPromise;
}
