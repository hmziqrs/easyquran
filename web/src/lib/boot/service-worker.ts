const SW_URL = "/service-worker.js";

const SW_OPTIONS: RegistrationOptions = {
  scope: "/",
  type: import.meta.env.DEV ? "module" : "classic",
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

export function startServiceWorker(): () => void {
  if (import.meta.env.PROD) void registerServiceWorker();
  return () => {};
}
