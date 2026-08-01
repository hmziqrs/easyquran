export function startCrashReporting(): () => void {
  const reportException = (description: string): void =>
    void import("$lib/firebase/analytics")
      .then(({ logException }) => logException(description, true))
      .catch(() => {});

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
