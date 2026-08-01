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

async function ready(): Promise<Analytics | null> {
  if (!analytics) await initAnalytics();
  return analytics;
}

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

export function pageView(path: string, title?: string): Promise<void> {
  const screen = path === "/" ? "home" : path.replace(/^\//, "").replace(/\/$/, "") || "home";
  return track("screen_view", {
    firebase_screen: screen,
    firebase_screen_class: screen,
    page_title: title ?? screen,
    page_location: `${location.origin}${path}`,
  });
}

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

/**
 * Log an exception to GA4 — the web-native crash signal. Crashlytics has no web
 * SDK, so this (GA4's reserved `exception` event) is the closest Firebase-native
 * equivalent. Consent-gated like every analytics call: track() drops the event
 * while collection is disabled. GA4 truncates `description` to 100 chars.
 *
 *   try { … } catch (err) { void logException(`foo failed: ${err}`, true); }
 */
export async function logException(description: string, fatal = false): Promise<void> {
  await track("exception", { description, fatal });
}
