import { browser } from "$app/environment";
import type { Analytics, ConsentSettings, AnalyticsCallOptions } from "firebase/analytics";

import { isConfigured, initApp, ANALYTICS_DEBUG } from "./index";

type AnalyticsParams = Record<string, string | number | boolean | null | undefined>;

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

async function withAnalytics(
  fn: (a: Analytics, mod: typeof import("firebase/analytics")) => void,
): Promise<void> {
  const a = await ready();
  if (!a) return;
  try {
    const mod = await import("firebase/analytics");
    fn(a, mod);
  } catch (err) {
    console.warn("[firebase] analytics call failed:", err);
  }
}

export function track(
  name: string,
  params?: AnalyticsParams,
  options?: AnalyticsCallOptions,
): Promise<void> {
  return withAnalytics((a, m) => {
    m.logEvent(a, name, params, options);
    if (ANALYTICS_DEBUG) console.debug("[firebase] event:", name, params ?? {});
  });
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

export function setCurrentScreen(name: string): Promise<void> {
  return withAnalytics((a, m) => m.setCurrentScreen(a, name));
}

export function setUserProperties(
  properties: AnalyticsParams,
  options?: AnalyticsCallOptions,
): Promise<void> {
  return withAnalytics((a, m) => m.setUserProperties(a, properties, options));
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

export function setAnalyticsCollectionEnabled(enabled: boolean): Promise<void> {
  return withAnalytics((a, m) => m.setAnalyticsCollectionEnabled(a, enabled));
}

export async function logException(description: string, fatal = false): Promise<void> {
  await track("exception", { description, fatal });
}
