import { browser } from "$app/environment";

class OnlineStore {
  #online = $state(browser ? navigator.onLine : true);
  #hydrated = $state(false);
  #cleanup: (() => void) | null = null;

  get online(): boolean {
    return this.#online;
  }

  get hydrated(): boolean {
    return this.#hydrated;
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#online = navigator.onLine;
    const onOnline = (): void => {
      this.#online = true;
    };
    const onOffline = (): void => {
      this.#online = false;
    };
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    this.#cleanup = () => {
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
    };
  }

  dispose(): void {
    this.#cleanup?.();
    this.#cleanup = null;
    this.#hydrated = false;
  }
}

export function createOnline(): OnlineStore {
  return new OnlineStore();
}

export const online = createOnline();
