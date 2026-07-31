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

/**
 * Start Firebase Analytics + Performance (gated by consent) and register the
 * consent bridge. Returns a teardown that detaches the consent listener.
 */
export function startAnalytics(): () => void {
  // fbAnalytics stays undefined if the dynamic import or init fails; the
  // consent bridge below is then a safe no-op, so a later retry or manual
  // consent change isn't silently lost.
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

  // Register the consent bridge SYNCHRONOUSLY, outside the async init, so it
  // (a) is always removable via the returned teardown and (b) survives an init
  // failure (applyConsent no-ops until fbAnalytics is assigned).
  window.addEventListener("easyquran:consent", applyConsent);

  void (async () => {
    try {
      fbAnalytics = await import("$lib/firebase/analytics");
      const fbPerf = await import("$lib/firebase/performance");

      // Start analytics + performance, gated by consent. (Performance flags
      // are honored at init; analytics can be toggled freely at runtime.)
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
      // Analytics is best-effort — a failed dynamic import or init must not
      // throw unhandled or disable the consent bridge registered above.
      console.warn("[firebase] init failed:", err);
    }
  })();

  return () => {
    window.removeEventListener("easyquran:consent", applyConsent);
  };
}
