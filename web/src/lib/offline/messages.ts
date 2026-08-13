export const SKIP_WAITING = "SKIP_WAITING" as const;
export const APP_READY = "APP_READY" as const;
export const UPDATE_TAKEOVER = "UPDATE_TAKEOVER" as const;
export const PREPARE_RELOAD = "PREPARE_RELOAD" as const;
export const PURGE_USER_CACHES = "PURGE_USER_CACHES" as const;
export const PURGE_ACK = "PURGE_ACK" as const;

export const SW_BROADCAST_CHANNEL = "easyquran-sw";
export const UPDATE_BROADCAST_CHANNEL = "easyquran-update";
export const PREPARE_RELOAD_EVENT = "easyquran-prepare-reload";

export type ClientToSwMessage =
  | { type: typeof SKIP_WAITING }
  | { type: typeof APP_READY }
  | { type: typeof PURGE_USER_CACHES };

export type SwToClientMessage =
  | { type: typeof UPDATE_TAKEOVER; version: string }
  | { type: typeof PURGE_ACK };

export type ClientToClientMessage = { type: typeof PREPARE_RELOAD };

export type UpdateMessage = SwToClientMessage | ClientToClientMessage;

const DEFAULT_PURGE_TIMEOUT_MS = 5000;

export function purgeUserCaches(timeoutMs: number = DEFAULT_PURGE_TIMEOUT_MS): Promise<void> {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- navigator is a browser-only global; this file is imported by both client and service-worker bundles, so importing $app/environment (the usual SSR guard) would break SW bundling.
  if (typeof navigator === "undefined") return Promise.resolve();
  const ctrl = navigator.serviceWorker?.controller;
  if (!ctrl) return Promise.resolve();
  return new Promise<void>((resolve) => {
    let settled = false;
    const channel = new MessageChannel();
    const finish = (): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        channel.port1.close();
      } catch {}
      try {
        channel.port2.close();
      } catch {}
      resolve();
    };
    const timer = setTimeout(finish, timeoutMs);
    channel.port1.onmessage = (): void => finish();
    try {
      ctrl.postMessage({ type: PURGE_USER_CACHES }, [channel.port2]);
    } catch {
      finish();
    }
  });
}
