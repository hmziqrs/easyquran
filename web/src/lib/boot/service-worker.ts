const SW_URL = "/service-worker.js";

const SW_OPTIONS: RegistrationOptions = {
  scope: "/",
  // Kit 3 always emits the service worker as an ES module (the SW build's
  // rolldown output format is 'es'), so dev and prod both register it as a
  // module — a 'classic' registration would fail evaluation in production.
  type: "module",
  updateViaCache: "none",
};

let registration: Promise<ServiceWorkerRegistration | null> | null = null;

export function registerServiceWorker(): Promise<ServiceWorkerRegistration | null> {
  if (!("serviceWorker" in navigator)) return Promise.resolve(null);
  registration ??= navigator.serviceWorker.register(SW_URL, SW_OPTIONS).catch((err) => {
    console.warn("[sw] registration failed:", err);
    registration = null;
    return null;
  });
  return registration;
}
