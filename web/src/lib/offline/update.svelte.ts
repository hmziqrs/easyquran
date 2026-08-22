import { browser } from "$app/environment";
import { updated } from "$app/state";
import { registerServiceWorker } from "$lib/boot/service-worker";
import {
  PREPARE_RELOAD,
  PREPARE_RELOAD_EVENT,
  SKIP_WAITING,
  SW_BROADCAST_CHANNEL,
  UPDATE_BROADCAST_CHANNEL,
  UPDATE_TAKEOVER,
} from "$lib/offline/messages";
import { readRaw, removeRaw, writeRaw } from "$lib/storage";

const RELOAD_GUARD = "easyquran.reload-guard";
const PAINT_KEY = "easyquran.update.waiting";
const UPDATE_CHECK_INTERVAL_MS = 5 * 60_000;

class UpdateStore {
  #waiting = $state(false);
  #hydrated = false;
  #reloadArmed = false;
  #registration: ServiceWorkerRegistration | null = null;
  #registrationResolved = false;
  #channel: BroadcastChannel | null = null;
  #cleanups: Array<() => void> = [];
  #lastUpdateCheckAt: number | null = null;
  #updateCheckInFlight: Promise<void> | null = null;
  #versionFallbackWired = false;

  get waiting(): boolean {
    return this.#waiting;
  }

  get available(): boolean {
    if (!browser) return false;
    if (this.#waiting) return true;
    return !("serviceWorker" in navigator) && updated.current;
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;

    if (readRaw("session", RELOAD_GUARD) === "1") this.#reloadArmed = true;

    if ("BroadcastChannel" in globalThis) {
      this.#channel = new BroadcastChannel(UPDATE_BROADCAST_CHANNEL);
      this.#channel.addEventListener("message", (event) => {
        if (event.data?.type === PREPARE_RELOAD) this.#armReloadGuard();
      });
      this.#cleanups.push(() => this.#channel?.close());

      const swChannel = new BroadcastChannel(SW_BROADCAST_CHANNEL);
      swChannel.addEventListener("message", (event) => {
        if (event.data?.type === UPDATE_TAKEOVER) this.#evaluateReload();
      });
      this.#cleanups.push(() => swChannel.close());
    }

    const painted = readRaw("local", PAINT_KEY) === "1";
    if (painted) this.#waiting = true;

    if ("serviceWorker" in navigator) {
      void this.#wireServiceWorker();
    } else {
      this.#wireVersionFallback();
    }
  }

  async #wireServiceWorker(): Promise<void> {
    const reg = await this.#getRegistration();
    if (!reg) {
      this.#wireVersionFallback();
      return;
    }
    this.#syncWaiting(reg);
    this.#lastUpdateCheckAt = Date.now();
    reg.addEventListener("updatefound", this.#onUpdateFound);

    const onControllerChange = (): void => {
      const ctrl = navigator.serviceWorker.controller;
      if (ctrl) this.#evaluateReload();
    };
    navigator.serviceWorker.addEventListener("controllerchange", onControllerChange);

    const onVisibility = (): void => {
      if (document.visibilityState !== "visible") return;
      void this.#checkServiceWorkerUpdate();
    };
    document.addEventListener("visibilitychange", onVisibility);

    this.#cleanups.push(
      () => reg.removeEventListener("updatefound", this.#onUpdateFound),
      () => navigator.serviceWorker.removeEventListener("controllerchange", onControllerChange),
      () => document.removeEventListener("visibilitychange", onVisibility),
    );
  }

  #wireVersionFallback(): void {
    if (this.#versionFallbackWired) return;
    this.#versionFallbackWired = true;
    const check = (): void => {
      void updated.check().catch(() => {});
    };
    const onVisibility = (): void => {
      if (document.visibilityState === "visible") check();
    };
    check();
    document.addEventListener("visibilitychange", onVisibility);
    this.#cleanups.push(() => document.removeEventListener("visibilitychange", onVisibility));
  }

  #checkServiceWorkerUpdate(): Promise<void> {
    if (this.#updateCheckInFlight) return this.#updateCheckInFlight;
    const now = Date.now();
    if (
      this.#lastUpdateCheckAt !== null &&
      now - this.#lastUpdateCheckAt < UPDATE_CHECK_INTERVAL_MS
    ) {
      return Promise.resolve();
    }
    this.#lastUpdateCheckAt = now;
    const run = (async (): Promise<void> => {
      const reg = await this.#getRegistration();
      if (!reg) return;
      try {
        await reg.update();
      } catch {}
      this.#syncWaiting(reg);
    })();
    this.#updateCheckInFlight = run;
    const clearInFlight = (): void => {
      if (this.#updateCheckInFlight === run) this.#updateCheckInFlight = null;
    };
    void run.then(clearInFlight, clearInFlight);
    return run;
  }

  readonly #onUpdateFound = (): void => {
    const installing = this.#registration?.installing;
    if (!installing) {
      this.#syncWaiting(this.#registration);
      return;
    }
    installing.addEventListener("statechange", () => {
      this.#syncWaiting(this.#registration);
    });
  };

  #syncWaiting(reg: ServiceWorkerRegistration | null | undefined): void {
    const next = reg?.waiting != null;
    this.#waiting = next;
    this.#mirrorWaiting();
  }

  #mirrorWaiting(): void {
    if (!browser) return;
    if (this.#waiting) writeRaw("local", PAINT_KEY, "1");
    else removeRaw("local", PAINT_KEY);
  }

  async #getRegistration(): Promise<ServiceWorkerRegistration | null> {
    if (this.#registrationResolved) return this.#registration;
    this.#registrationResolved = true;
    this.#registration = await registerServiceWorker();
    return this.#registration;
  }

  #armReloadGuard(): void {
    this.#reloadArmed = true;
    writeRaw("session", RELOAD_GUARD, "1");
  }

  #evaluateReload(): void {
    if (!this.#reloadArmed) return;
    this.#reloadArmed = false;
    removeRaw("session", RELOAD_GUARD);
    this.#waiting = false;
    this.#mirrorWaiting();
    window.location.reload();
  }

  apply(): void {
    const waiting = this.#registration?.waiting;
    if (waiting) {
      this.#armReloadGuard();
      this.#channel?.postMessage({ type: PREPARE_RELOAD });
      try {
        window.dispatchEvent(new CustomEvent(PREPARE_RELOAD_EVENT));
      } catch {}
      waiting.postMessage({ type: SKIP_WAITING });
      return;
    }
    if (!("serviceWorker" in navigator) && updated.current) window.location.reload();
  }

  dispose(): void {
    for (const cleanup of this.#cleanups) cleanup();
    this.#cleanups = [];
    this.#channel = null;
    this.#registration = null;
    this.#registrationResolved = false;
    this.#lastUpdateCheckAt = null;
    this.#updateCheckInFlight = null;
    this.#versionFallbackWired = false;
    this.#hydrated = false;
  }
}

export function createUpdate(): UpdateStore {
  return new UpdateStore();
}

export const update = createUpdate();
