/* ════════════════════════════════════════════════════════════════════════
   firebase/index.ts — shared core: web config + the single FirebaseApp.

   Everything browser-only here is loaded LAZILY via dynamic import from inside
   initApp(), so neither firebase/app nor any feature module ships in the page's
   critical modulepreload graph or runs during SSR/prerender. Every entry point
   is guarded and wrapped in try/catch — nothing here can take the page down.

   The web config is *not* a secret — by Firebase's design it ships in the
   client bundle; access is gated by Security Rules / App Check, not by hiding
   it. It's read from PUBLIC_FIREBASE_* env vars (see .env.example) so the same
   source builds against different Firebase projects. With no config present,
   every function below quietly no-ops.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { env } from "$env/dynamic/public";
import type { FirebaseApp, FirebaseOptions } from "firebase/app";

export const firebaseConfig: FirebaseOptions = {
  apiKey: env.PUBLIC_FIREBASE_API_KEY,
  authDomain: env.PUBLIC_FIREBASE_AUTH_DOMAIN,
  projectId: env.PUBLIC_FIREBASE_PROJECT_ID,
  storageBucket: env.PUBLIC_FIREBASE_STORAGE_BUCKET,
  messagingSenderId: env.PUBLIC_FIREBASE_MESSAGING_SENDER_ID,
  appId: env.PUBLIC_FIREBASE_APP_ID,
  measurementId: env.PUBLIC_FIREBASE_MEASUREMENT_ID,
};

/** True once the env vars are filled in. Until then everything no-ops. */
export const isConfigured = Boolean(firebaseConfig.apiKey && firebaseConfig.appId);

/** Web Push VAPID key (Firebase console → Project settings → Cloud Messaging → Web config). */
export const FCM_VAPID_KEY = env.PUBLIC_FIREBASE_VAPID_KEY;

/**
 * Backend API origin for device-token registration (the Axum `/device/v1/*`
 * routes). Trailing slash trimmed. Empty until accounts arrive — without it the
 * client still obtains an FCM token and keeps it locally, so a later login can
 * register it. Optional: if set, the messaging module POSTs the token to
 * `${API_BASE_URL}/device/v1/register` on subscribe.
 */
export const API_BASE_URL = (env.PUBLIC_API_BASE_URL || "").replace(/\/+$/, "");

/** When true, analytics calls are echoed to the console (local dev aid). For
 *  GA4 DebugView itself, append `?firebase_debug_mode=1` to the URL instead. */
export const ANALYTICS_DEBUG = env.PUBLIC_FIREBASE_ANALYTICS_DEBUG === "true";

let app: FirebaseApp | null = null;
let appPromise: Promise<FirebaseApp | null> | null = null;

/** The shared FirebaseApp once initApp() has resolved, else null. */
export function getFirebaseApp(): FirebaseApp | null {
  return app;
}

/**
 * Initialize the shared FirebaseApp exactly once. Browser-only; no-op during
 * SSR and when unconfigured. Every feature module (analytics, performance,
 * messaging) calls this and awaits the same promise.
 */
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
