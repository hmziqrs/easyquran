import { consent } from "$lib/stores/consent.svelte";

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
      const fbPerf = await import("$lib/firebase/performance");

      await fbAnalytics.initAnalytics();
      void fbPerf.initPerformance({
        dataCollectionEnabled: consent.performance,
        instrumentationEnabled: consent.performance,
      });
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
