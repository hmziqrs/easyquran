/* ════════════════════════════════════════════════════════════════════════
   firebase.ts — Firebase Analytics, wired for SvelteKit.

   `initializeApp` is pure config and safe on the server. Analytics is
   browser-only — it injects gtag.js and touches window/cookies/localStorage —
   so we init the app lazily but start analytics only in the browser, gated on
   `isSupported()`. Nothing here runs during SSR, and nothing here can take the
   page down: every entry point is guarded and wrapped in try/catch.

   The web config is *not* a secret — by Firebase's design it ships in the
   client bundle; access is gated by Security Rules / App Check, not by hiding
   it. It's read from PUBLIC_FIREBASE_* env vars (see .env.example) so the same
   source builds against different Firebase projects. With no config present,
   every function below quietly no-ops.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { env } from "$env/dynamic/public";
import { getApps, getApp, initializeApp, type FirebaseApp } from "firebase/app";
import type { Analytics } from "firebase/analytics";

const firebaseConfig = {
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

let cachedApp: FirebaseApp | null = null;

/**
 * The shared FirebaseApp, created on first use. Returns null when the project
 * isn't configured yet, so callers never have to special-case a half-built env.
 * Keeps a single instance across hot reloads and repeated imports.
 */
export function getFirebaseApp(): FirebaseApp | null {
  if (!isConfigured) return null;
  if (!cachedApp) {
    cachedApp = getApps().length ? getApp() : initializeApp(firebaseConfig);
  }
  return cachedApp;
}

let analytics: Analytics | null = null;
let initPromise: Promise<Analytics | null> | null = null;

/** Start analytics in the browser, exactly once. No-op during SSR. */
export function initAnalytics(): Promise<Analytics | null> {
  if (!browser) return Promise.resolve(null);
  const app = getFirebaseApp();
  if (!app) return Promise.resolve(null);
  if (!initPromise) {
    initPromise = (async () => {
      try {
        const { isSupported, getAnalytics } = await import("firebase/analytics");
        if (await isSupported()) analytics = getAnalytics(app);
      } catch (err) {
        console.warn("[firebase] analytics failed to start:", err);
      }
      return analytics;
    })();
  }
  return initPromise;
}

/**
 * Fire a custom event once analytics is ready. Safe to call from anywhere at
 * any time — it no-ops until `initAnalytics()` has resolved and silently drops
 * failures. Use reserved names like "page_view" sparingly.
 */
export async function track(name: string, params?: Record<string, unknown>): Promise<void> {
  if (!analytics) return;
  try {
    const { logEvent } = await import("firebase/analytics");
    logEvent(analytics, name, params);
  } catch (err) {
    console.warn("[firebase] logEvent failed:", err);
  }
}
