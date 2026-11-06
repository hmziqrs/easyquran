import { consent } from "#lib/stores/consent.svelte.js";

// Firebase Performance is intentionally NOT initialised: nothing in the app
// consumes its traces, and its bundled web-vitals auto-instrumentation crashes
// with "Cannot read properties of undefined (reading 'startTime')" from
// reportAllChanges in some browsers. Re-enable only together with real trace
// consumers (see #lib/firebase/performance.ts).

/** Loads Firebase Analytics only once consent is true; nothing is fetched before that. */
export function startAnalytics(): () => void {
  let fbAnalytics: typeof import("#lib/firebase/analytics.js") | undefined;
  let loading = false;

  const load = async (): Promise<void> => {
    if (loading) return;
    loading = true;
    try {
      fbAnalytics = await import("#lib/firebase/analytics.js");
      await fbAnalytics.initAnalytics();
      applyConsent();
      if (consent.analytics) void fbAnalytics.pageView(location.pathname);
    } catch (err) {
      console.warn("[firebase] init failed:", err);
    }
  };

  function applyConsent(): void {
    if (!fbAnalytics) {
      if (consent.analytics) void load();
      return;
    }
    void fbAnalytics.setConsentState(consent.consentSettings);
    void fbAnalytics.setAnalyticsCollectionEnabled(consent.analytics);
  }

  window.addEventListener("easyquran:consent", applyConsent);
  applyConsent();

  return () => {
    window.removeEventListener("easyquran:consent", applyConsent);
  };
}
