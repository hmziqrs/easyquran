import { consent } from "$lib/stores/consent.svelte";

// Firebase Performance is intentionally NOT initialised: nothing in the app
// consumes its traces, and its bundled web-vitals auto-instrumentation crashes
// with "Cannot read properties of undefined (reading 'startTime')" from
// reportAllChanges in some browsers. Re-enable only together with real trace
// consumers (see $lib/firebase/performance.ts).

export function startAnalytics(): () => void {
  let fbAnalytics: typeof import("$lib/firebase/analytics") | undefined;

  const applyConsent = (): void => {
    if (!fbAnalytics) return;
    void fbAnalytics.setConsentState(consent.consentSettings);
    void fbAnalytics.setAnalyticsCollectionEnabled(consent.analytics);
  };

  window.addEventListener("easyquran:consent", applyConsent);

  void (async () => {
    try {
      fbAnalytics = await import("$lib/firebase/analytics");

      await fbAnalytics.initAnalytics();
      applyConsent();

      void fbAnalytics.pageView(location.pathname);
    } catch (err) {
      console.warn("[firebase] init failed:", err);
    }
  })();

  return () => {
    window.removeEventListener("easyquran:consent", applyConsent);
  };
}
