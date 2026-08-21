import { browser } from "$app/environment";
import { isConfigured } from "$lib/firebase";
import { track } from "$lib/firebase/analytics";
import {
  deleteFcmToken,
  getFcmToken,
  getPermissionState,
  initMessaging,
  isMessagingSupported,
  onForegroundMessage,
  registerTokenWithServer,
  requestPermission,
  unregisterTokenFromServer,
  type PermissionState,
} from "$lib/firebase/messaging";
import { asObject, asString, readJSON, writeJSON } from "$lib/storage";
import type { MessagePayload } from "firebase/messaging";

const STORAGE_KEY = "easyquran.fcm";

interface PersistedFcm {
  token: string | null;
  subscribed: boolean;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON → JSON.parse); this function is the parser (asObject/asString validate fields)
export function decodeFcm(raw: unknown): PersistedFcm {
  const stored = asObject(raw);
  if (!stored) return { token: null, subscribed: false };
  return {
    token: asString(stored.token) ?? null,
    subscribed: stored.subscribed === true,
  };
}

class NotificationsStore {
  #permission = $state<PermissionState>("default");
  #supported = $state<boolean | null>(null);
  #subscribed = $state(false);
  #token = $state<string | null>(null);
  #lastMessage = $state.raw<MessagePayload | null>(null);
  #messageSeq = $state(0);
  #busy = $state(false);
  #hydrated = false;
  #generation = 0;
  #refreshInFlight: Promise<void> | null = null;
  #foregroundUnsubscribe: (() => void) | null = null;
  #visibilityListener: (() => void) | null = null;

  get permission(): PermissionState {
    return this.#permission;
  }
  get supported(): boolean | null {
    return this.#supported;
  }
  get subscribed(): boolean {
    return this.#subscribed;
  }
  get busy(): boolean {
    return this.#busy;
  }
  get lastMessage(): MessagePayload | null {
    return this.#lastMessage;
  }
  get messageSeq(): number {
    return this.#messageSeq;
  }
  get canSubscribe(): boolean {
    return this.#supported === true && this.#permission !== "denied";
  }
  get statusText(): string {
    if (!isConfigured) return "Notifications unavailable (not configured).";
    if (this.#supported === null) return "Checking…";
    if (this.#supported === false) return "Not supported on this browser.";
    if (this.#permission === "denied") return "Blocked in your browser settings.";
    if (this.#subscribed) return "On — you'll receive notifications.";
    if (this.#permission === "default") return "Off — enable to get updates.";
    return "Off.";
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;

    const stored = decodeFcm(readJSON(STORAGE_KEY));
    this.#token = stored.token;
    this.#subscribed = stored.subscribed;
    this.#permission = getPermissionState();

    if (this.#permission !== "granted") this.#subscribed = false;

    void this.#hydrateMessaging();
  }

  async #hydrateMessaging(): Promise<void> {
    const generation = this.#generation;
    const supported = await isMessagingSupported();
    if (!this.#hydrated || generation !== this.#generation) return;
    this.#supported = supported;
    if (!supported) {
      this.#subscribed = false;
      return;
    }
    await this.#wireListeners();
    if (this.#subscribed && this.#permission === "granted") await this.#refreshToken();
  }

  async #wireListeners(): Promise<void> {
    if (!browser) return;
    const unsubscribe = await onForegroundMessage((payload) => {
      this.#lastMessage = payload;
      this.#messageSeq += 1;
      void track("notification_received_foreground", { message_id: payload.messageId });
    });
    if (!this.#hydrated) {
      unsubscribe();
      return;
    }
    this.#foregroundUnsubscribe = unsubscribe;
    const onVisibility = (): void => {
      if (document.visibilityState === "visible" && this.#subscribed) {
        void this.#refreshToken();
      }
    };
    this.#visibilityListener = onVisibility;
    document.addEventListener("visibilitychange", onVisibility);
  }

  #refreshToken(): Promise<void> {
    if (this.#refreshInFlight) return this.#refreshInFlight;
    const run = (async (): Promise<void> => {
      this.#permission = getPermissionState();
      const gen = this.#generation;
      const token = await getFcmToken();
      if (gen !== this.#generation) return;
      if (!token) {
        if (this.#permission !== "granted") this.#setSubscribed(false, null);
        return;
      }
      if (token === this.#token) {
        this.#setSubscribed(true, token);
        return;
      }
      const registered = await registerTokenWithServer(token);
      if (gen !== this.#generation) return;
      this.#token = token;
      this.#setSubscribed(true, token, registered);
      void track("notification_token_refresh");
    })();
    this.#refreshInFlight = run;
    const clearInFlight = (): void => {
      if (this.#refreshInFlight === run) this.#refreshInFlight = null;
    };
    void run.then(clearInFlight, clearInFlight);
    return run;
  }

  #persist(): void {
    if (!browser) return;
    writeJSON(STORAGE_KEY, { token: this.#token, subscribed: this.#subscribed });
  }

  #setSubscribed(subscribed: boolean, token: string | null, registered = true): void {
    this.#subscribed = subscribed;
    this.#token = token;
    this.#persist();
    if (!registered && token) {
      console.info("[firebase] token kept locally; backend registration pending auth/config.");
    }
  }

  async subscribe(): Promise<boolean> {
    if (!browser || this.#busy) return false;
    if (this.#supported === false) return false;
    this.#busy = true;
    this.#generation += 1;
    try {
      const permission = await requestPermission();
      this.#permission = permission;
      if (permission !== "granted") return false;

      const token = await getFcmToken();
      if (!token) return false;

      const registered = await registerTokenWithServer(token);
      this.#setSubscribed(true, token, registered);
      void track("notification_subscribe", { registered });
      return true;
    } catch (err) {
      console.warn("[firebase] subscribe failed:", err);
      return false;
    } finally {
      this.#busy = false;
    }
  }

  async unsubscribe(): Promise<boolean> {
    if (!browser || this.#busy) return false;
    this.#busy = true;
    this.#generation += 1;
    if (this.#refreshInFlight) await this.#refreshInFlight.catch(() => {});
    try {
      if (this.#token) await unregisterTokenFromServer(this.#token);
      await deleteFcmToken();
      this.#setSubscribed(false, null);
      void track("notification_unsubscribe");
      return true;
    } catch (err) {
      console.warn("[firebase] unsubscribe failed:", err);
      return false;
    } finally {
      this.#busy = false;
    }
  }

  clearMessage(): void {
    this.#lastMessage = null;
  }

  syncPermission(): void {
    if (!browser) return;
    this.#permission = getPermissionState();
    if (this.#permission !== "granted") {
      this.#setSubscribed(false, null);
    }
    if (this.#supported === true) void initMessaging();
  }

  dispose(): void {
    this.#generation += 1;
    this.#foregroundUnsubscribe?.();
    this.#foregroundUnsubscribe = null;
    if (this.#visibilityListener) {
      document.removeEventListener("visibilitychange", this.#visibilityListener);
      this.#visibilityListener = null;
    }
    this.#hydrated = false;
  }
}

export function createNotifications(): NotificationsStore {
  return new NotificationsStore();
}

export const notifications = createNotifications();
