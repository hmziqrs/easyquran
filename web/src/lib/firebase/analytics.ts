/* ════════════════════════════════════════════════════════════════════════
   firebase/analytics.ts — Firebase Analytics (GA4).

   Browser-only and loaded LAZILY via dynamic import from initAnalytics(), so
   firebase/analytics never ships on the critical path or runs during SSR. All
   public functions are safe to call from anywhere at any time — they no-op
   until analytics is ready and silently drop failures. Respect the consent
   store (see stores/consent.svelte.ts): collection can be toggled at runtime
   and consent-mode state is applied on init.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { Analytics, ConsentSettings, AnalyticsCallOptions } from "firebase/analytics";
import { isConfigured, initApp, ANALYTICS_DEBUG } from "./index";

let analytics: Analytics | null = null;
let initPromise: Promise<Analytics | null> | null = null;

/** Start analytics in the browser, exactly once. No-op during SSR/unconfigured. */
export function initAnalytics(): Promise<Analytics | null> {
  if (!browser || !isConfigured) return Promise.resolve(null);
  if (!initPromise) {
    initPromise = (async () => {
      try {
        const core = await initApp();
        if (!core) return null;
        const { isSupported, getAnalytics } = await import("firebase/analytics");
        // isSupported() rejects in private mode / without cookies / no IndexedDB.
        if (!(await isSupported())) return null;
        analytics = getAnalytics(core);
      } catch (err) {
        console.warn("[firebase] analytics failed to start:", err);
      }
      return analytics;
    })();
  }
  return initPromise;
}

/** Resolve analytics once ready (resolves null if unsupported/unconfigured). */
async function ready(): Promise<Analytics | null> {
  if (!analytics) await initAnalytics();
  return analytics;
}

/**
 * Fire a custom event once analytics is ready. No-op until init resolves and
 * silently drops failures. Avoid reserved names unless intentional.
 */
export async function track(
  name: string,
  params?: Record<string, unknown>,
  options?: AnalyticsCallOptions,
): Promise<void> {
  const a = await ready();
  if (!a) return;
  try {
    const { logEvent } = await import("firebase/analytics");
    logEvent(a, name, params, options);
    if (ANALYTICS_DEBUG) console.debug("[firebase] event:", name, params ?? {});
  } catch (err) {
    console.warn("[firebase] logEvent failed:", err);
  }
}

/**
 * Log a page/screen view. Called on the initial load and on every client-side
 * navigation (see +layout.svelte's afterNavigate). Uses GA4's reserved
 * `screen_view` with `firebase_screen` params, plus a semantic `page_view`.
 */
export function pageView(path: string, title?: string): Promise<void> {
  const screen = path === "/" ? "home" : path.replace(/^\//, "").replace(/\/$/, "") || "home";
  return track("screen_view", {
    firebase_screen: screen,
    firebase_screen_class: screen,
    page_title: title ?? screen,
    page_location: `${location.origin}${path}`,
  });
}

/** Set the current screen name without sending an event. */
export async function setCurrentScreen(name: string): Promise<void> {
  const a = await ready();
  if (!a) return;
  try {
    const { setCurrentScreen } = await import("firebase/analytics");
    setCurrentScreen(a, name);
  } catch (err) {
    console.warn("[firebase] setCurrentScreen failed:", err);
  }
}

/** Attach arbitrary user properties (e.g. once an account exists). */
export async function setUserProperties(
  properties: Record<string, unknown>,
  options?: AnalyticsCallOptions,
): Promise<void> {
  const a = await ready();
  if (!a) return;
  try {
    const { setUserProperties } = await import("firebase/analytics");
    setUserProperties(a, properties, options);
  } catch (err) {
    console.warn("[firebase] setUserProperties failed:", err);
  }
}

/**
 * Apply Google consent-mode v2 state. Call before/after init — Firebase queues
 * it against the global gtag, so events fired while "denied" are dropped.
 */
export async function setConsentState(settings: ConsentSettings): Promise<void> {
  if (!browser || !isConfigured) return;
  try {
    const { setConsent } = await import("firebase/analytics");
    setConsent(settings);
  } catch (err) {
    console.warn("[firebase] setConsent failed:", err);
  }
}

/**
 * Enable/disable analytics collection at runtime (user opt-out). When disabled,
 * no events are sent; re-enabling resumes collection without losing the session.
 */
export async function setAnalyticsCollectionEnabled(enabled: boolean): Promise<void> {
  const a = await ready();
  if (!a) return;
  try {
    const { setAnalyticsCollectionEnabled } = await import("firebase/analytics");
    setAnalyticsCollectionEnabled(a, enabled);
  } catch (err) {
    console.warn("[firebase] setAnalyticsCollectionEnabled failed:", err);
  }
}
