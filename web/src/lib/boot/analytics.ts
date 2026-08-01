/* ════════════════════════════════════════════════════════════════════════
   boot/analytics.ts — start Firebase Analytics + Performance and bridge consent.

   Firebase is lazy-imported here (after hydration) so the SDK never enters the
   critical modulepreload graph — it starts only once the page is interactive.

   This module owns the consent bridge: the "easyquran:consent" window event
   (dispatched by stores/consent.svelte.ts whenever a flag changes) is applied
   to GA4 consent mode + the analytics collection toggle. The listener is
   registered SYNCHRONOUSLY (not inside the async init) and the returned
   teardown removes it — fixing the earlier leak where the listener was added
   inside a fire-and-forget IIFE and never removed. applyConsent stays a no-op
   until the analytics module has loaded, so a failed init is not silently lost:
   a later retry or manual consent change still flows through once it loads.
   ════════════════════════════════════════════════════════════════════════ */

import { consent } from "$lib/stores/consent.svelte";

export function startAnalytics(): () => void {
  let fbAnalytics: typeof import("$lib/firebase/analytics") | undefined;

  // Push the user's consent choices into GA4 consent mode + the analytics
  // collection toggle, and re-apply whenever they change (Settings panel).
  // (Performance has no runtime toggle — see lib/firebase/performance.ts —
  // it's consent-gated only at init below; the control reloads to apply it.)
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

      // First-load page view (after consent is applied, so the first event
      // respects the user's consent-mode state). Subsequent navigations are
      // logged from +layout.svelte's afterNavigate.
      void fbAnalytics.pageView(location.pathname);
    } catch (err) {
      console.warn("[firebase] init failed:", err);
    }
  })();

  return () => {
    window.removeEventListener("easyquran:consent", applyConsent);
  };
}
