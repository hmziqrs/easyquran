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

export async function logException(description: string, fatal = false): Promise<void> {
  await track("exception", { description, fatal });
}
