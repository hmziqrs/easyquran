/* ════════════════════════════════════════════════════════════════════════
   firebase.ts — Firebase Analytics, wired for SvelteKit.

   Both the firebase/app core and firebase/analytics are browser-only and are
   loaded LAZILY via dynamic import from inside initAnalytics() — so neither
   ships in the page's critical modulepreload graph or runs during SSR. Nothing
   here can take the page down: every entry point is guarded and wrapped in
   try/catch.

   The web config is *not* a secret — by Firebase's design it ships in the
   client bundle; access is gated by Security Rules / App Check, not by hiding
   it. It's read from PUBLIC_FIREBASE_* env vars (see .env.example) so the same
   source builds against different Firebase projects. With no config present,
   every function below quietly no-ops.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { env } from "$env/dynamic/public";
import type { FirebaseApp } from "firebase/app";
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

let app: FirebaseApp | null = null;
let analytics: Analytics | null = null;
let initPromise: Promise<Analytics | null> | null = null;

/** The shared FirebaseApp once initAnalytics() has run, else null. */
export function getFirebaseApp(): FirebaseApp | null {
  return app;
}

/** Start analytics in the browser, exactly once. No-op during SSR. */
export function initAnalytics(): Promise<Analytics | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!initPromise) {
    initPromise = (async () => {
      try {
        // Dynamic-import the core + analytics so they only download after
        // hydration, never on the critical path.
        const { getApps, getApp, initializeApp } = await import("firebase/app");
        const { isSupported, getAnalytics } = await import("firebase/analytics");
        const core = getApps().length ? getApp() : initializeApp(firebaseConfig);
        app = core;
        if (await isSupported()) analytics = getAnalytics(core);
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
