/* ════════════════════════════════════════════════════════════════════════
   boot/service-worker.ts — register the root Service Worker.

   Registers /sw.js (app-shell + FCM) in production so the reader is offline-
   capable after first visit. Skipped in dev (it would cache over HMR).
   firebase/messaging.ts re-uses this same registration.

   The returned teardown is intentionally a no-op: a service worker registration
   is persistent across the page lifetime and navigations, and there is no
   listener to remove. The teardown exists so this service composes uniformly
   with the other boot services (analytics / crash-reporting / offline-engine).
   ════════════════════════════════════════════════════════════════════════ */

/**
 * Register the root service worker in production. No-op in dev. Returns a
 * no-op teardown (the registration has no listener to remove).
 */
export function startServiceWorker(): () => void {
  if (import.meta.env.PROD && "serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js", { scope: "/" }).catch((err) => {
      console.warn("[sw] registration failed:", err);
    });
  }
  return () => {
    /* persistent across the page lifetime — nothing to detach */
  };
}
