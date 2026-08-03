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

const RELOAD_GUARD = "easyquran.reload-guard";
const PAINT_KEY = "easyquran.update.waiting";

class UpdateStore {
  #waiting = $state(false);
  #hydrated = false;
  #reloadArmed = false;
  #registration: ServiceWorkerRegistration | null = null;
  #registrationResolved = false;
  #channel: BroadcastChannel | null = null;
  #cleanups: Array<() => void> = [];

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

    if (window.sessionStorage.getItem(RELOAD_GUARD) === "1") this.#reloadArmed = true;

    if (typeof BroadcastChannel !== "undefined") {
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

    const painted = window.localStorage.getItem(PAINT_KEY) === "1";
    if (painted) this.#waiting = true;

    const stopEffect = $effect.root(() => {
      $effect(() => {
        if (!updated.current) return;
        void this.#onVersionChanged();
      });
    });
    this.#cleanups.push(stopEffect);

    if ("serviceWorker" in navigator) {
      void this.#wireServiceWorker();
    }
  }

  async #wireServiceWorker(): Promise<void> {
    const reg = await this.#getRegistration();
    if (!reg) return;
    this.#syncWaiting(reg);
    reg.addEventListener("updatefound", this.#onUpdateFound);

    const onControllerChange = (): void => {
      const ctrl = navigator.serviceWorker.controller;
      if (ctrl) this.#evaluateReload();
    };
    navigator.serviceWorker.addEventListener("controllerchange", onControllerChange);

    const onVisibility = (): void => {
      if (document.visibilityState !== "visible") return;
      void updated.check();
      void this.#getRegistration().then((r) => {
        if (r) void r.update().catch(() => {});
      });
    };
    document.addEventListener("visibilitychange", onVisibility);

    this.#cleanups.push(
      () => reg.removeEventListener("updatefound", this.#onUpdateFound),
      () => navigator.serviceWorker.removeEventListener("controllerchange", onControllerChange),
      () => document.removeEventListener("visibilitychange", onVisibility),
    );
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

  async #onVersionChanged(): Promise<void> {
    const reg = await this.#getRegistration();
    if (!reg) {
      this.#mirrorWaiting();
      return;
    }
    try {
      await reg.update();
    } catch {}
    this.#syncWaiting(reg);
  }

  #syncWaiting(reg: ServiceWorkerRegistration | null | undefined): void {
    const next = reg?.waiting != null;
    this.#waiting = next;
    this.#mirrorWaiting();
  }

  #mirrorWaiting(): void {
    if (!browser) return;
    try {
      if (this.#waiting) window.localStorage.setItem(PAINT_KEY, "1");
      else window.localStorage.removeItem(PAINT_KEY);
    } catch {}
  }

  async #getRegistration(): Promise<ServiceWorkerRegistration | null> {
    if (this.#registrationResolved) return this.#registration;
    this.#registrationResolved = true;
    this.#registration = await registerServiceWorker();
    return this.#registration;
  }

  #armReloadGuard(): void {
    this.#reloadArmed = true;
    try {
      window.sessionStorage.setItem(RELOAD_GUARD, "1");
    } catch {}
  }

  #evaluateReload(): void {
    if (!this.#reloadArmed) return;
    this.#reloadArmed = false;
    try {
      window.sessionStorage.removeItem(RELOAD_GUARD);
    } catch {}
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
    if (updated.current) window.location.reload();
  }

  dispose(): void {
    for (const cleanup of this.#cleanups) cleanup();
    this.#cleanups = [];
    this.#channel = null;
    this.#registration = null;
    this.#registrationResolved = false;
    this.#hydrated = false;
  }
}

export function createUpdate(): UpdateStore {
  return new UpdateStore();
}

export const update = createUpdate();
