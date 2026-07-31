/* ════════════════════════════════════════════════════════════════════════
   boot/crash-reporting.ts — forward uncaught errors to GA4 as exceptions.

   Crashlytics has no web SDK, so GA4's reserved `exception` event is the
   Firebase-native equivalent. Consent-gated: logException -> track() drops the
   event while collection is off. The analytics module is dynamic-imported per
   report so it stays out of the critical bundle (errors are rare; the module
   caches after first load).

   The returned teardown detaches both window listeners.
   ════════════════════════════════════════════════════════════════════════ */

/**
 * Install global error + unhandledrejection handlers that report to GA4.
 * Returns a teardown that removes both listeners.
 */
export function startCrashReporting(): () => void {
  const reportException = (description: string): void =>
    void import("$lib/firebase/analytics")
      .then(({ logException }) => logException(description, true))
      .catch(() => {
        /* crash reporting is best-effort */
      });

  const onError = (event: ErrorEvent): void =>
    reportException(
      `Uncaught: ${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`,
    );

  const onRejection = (event: PromiseRejectionEvent): void => {
    const reason =
      event.reason instanceof Error
        ? `${event.reason.name}: ${event.reason.message}`
        : String(event.reason);
    reportException(`Unhandled rejection: ${reason}`);
  };

  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);

  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}
