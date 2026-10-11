import { browser } from "$app/env";
import type { MessagePayload } from "firebase/messaging";

import type { AuthStatus } from "#lib/auth/auth-state.svelte.js";
import { track } from "#lib/firebase/analytics.js";
import { isMessagingConfigured } from "#lib/firebase/index.js";
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
} from "#lib/firebase/messaging.js";
import { publicPushPayload } from "#lib/firebase/push-privacy.js";
import { asNumber, asObject, asString, readJSON, writeJSON } from "#lib/storage/index.js";

const STORAGE_KEY = "easyquran.fcm";

interface PersistedFcm {
  token: string | null;
  subscribed: boolean;
  ownerId: number | null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- untyped localStorage JSON boundary, validated by storage decoders
export function decodeFcm(raw: unknown): PersistedFcm {
  const stored = asObject(raw);
  if (!stored) return { token: null, subscribed: false, ownerId: null };
  const ownerId = asNumber(stored.ownerId, 1, 2_147_483_647);
  return {
    token: asString(stored.token) ?? null,
    subscribed: stored.subscribed === true,
    ownerId: ownerId !== undefined && Number.isSafeInteger(ownerId) && ownerId > 0 ? ownerId : null,
  };
}

class NotificationsStore {
  #permission = $state<PermissionState>("default");
  #supported = $state<boolean | null>(null);
  #configured = $state(isMessagingConfigured);
  #pushError = $state(false);
  #subscribed = $state(false);
  #token = $state<string | null>(null);
  #ownerId = $state<number | null>(null);
  #lastMessage = $state.raw<MessagePayload | null>(null);
  #messageSeq = $state(0);
  #busy = $state(false);
  #hydrated = false;
  #generation = 0;
  #stored: PersistedFcm | null = null;
  #refreshInFlight: Promise<void> | null = null;
  #teardownInFlight: Promise<void> = Promise.resolve();
  #registerController: AbortController | null = null;
  #registeringToken: string | null = null;
  #foregroundUnsubscribe: (() => void) | null = null;
  #visibilityListener: (() => void) | null = null;
  #controllerListener: (() => void) | null = null;

  get permission(): PermissionState {
    return this.#permission;
  }
  get supported(): boolean | null {
    return this.#supported;
  }
  get configured(): boolean {
    return this.#configured;
  }
  get pushError(): boolean {
    return this.#pushError;
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
    return this.#ownerId !== null && this.#supported === true && this.#permission !== "denied";
  }

  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;
    this.#stored = decodeFcm(readJSON(STORAGE_KEY));
    this.#permission = getPermissionState();
    this.#configured = isMessagingConfigured;
    if (this.#ownerId !== null && this.#stored.ownerId !== this.#ownerId) {
      if (this.#stored.token) this.#queueTeardown(this.#stored.token, this.#stored.ownerId);
      this.#stored = null;
      this.#persist();
    }
    this.#restoreOwnedSubscription();
    void this.#hydrateMessaging();
  }

  async onAuthChanged(status: AuthStatus, ownerId: number | null): Promise<void> {
    if (!browser) return;
    const nextOwner = status === "authenticated" ? ownerId : null;
    if (status === "unknown" && this.#ownerId === null) return;
    if (nextOwner === this.#ownerId && status !== "anonymous") return;
    const previousOwner = this.#ownerId;
    const previousToken = this.#token ?? this.#registeringToken;
    this.#invalidate();
    this.#ownerId = nextOwner;
    this.#lastMessage = null;
    this.#subscribed = false;
    this.#token = null;
    this.#busy = false;
    if (!this.#hydrated && !previousToken && !this.#stored) return;
    if (status === "unknown") return;
    const stored = this.#stored;
    if (previousToken) this.#queueTeardown(previousToken, previousOwner);
    if (stored?.token && stored.ownerId !== nextOwner) {
      if (stored.token !== previousToken) this.#queueTeardown(stored.token, stored.ownerId);
      this.#stored = null;
    }
    this.#restoreOwnedSubscription();
    this.#persist();
    await this.#teardownInFlight;
    if (this.#hydrated && this.#subscribed && this.#ownerId === nextOwner) {
      await this.#refreshToken();
    }
  }

  #restoreOwnedSubscription(): void {
    if (this.#ownerId === null || this.#stored?.ownerId !== this.#ownerId) return;
    if (this.#permission !== "granted") return;
    this.#token = this.#stored.token;
    this.#subscribed = this.#stored.subscribed;
  }

  #invalidate(): void {
    this.#generation += 1;
    this.#registerController?.abort();
    this.#registerController = null;
    this.#registeringToken = null;
    this.#refreshInFlight = null;
  }

  #queueTeardown(token: string, ownerId: number | null): void {
    this.#teardownInFlight = this.#teardownInFlight.then(async () => {
      if (ownerId !== null) await unregisterTokenFromServer(token, ownerId).catch(() => false);
      await deleteFcmToken().catch(() => {});
    });
  }

  async #hydrateMessaging(): Promise<void> {
    const supported = await isMessagingSupported();
    if (!this.#hydrated) return;
    this.#supported = supported;
    this.#wireRecoveryListeners();
    if (!supported) {
      this.#subscribed = false;
      return;
    }
    await this.#wireListeners();
    if (this.#subscribed && this.#permission === "granted") await this.#refreshToken();
  }

  #probeSupport(): void {
    void isMessagingSupported().then((ok) => {
      if (!this.#hydrated) return;
      this.#supported = ok;
      if (!ok) this.#subscribed = false;
    });
  }

  #wireRecoveryListeners(): void {
    if (!browser || this.#visibilityListener) return;
    this.#visibilityListener = (): void => {
      if (document.visibilityState !== "visible") return;
      if (this.#subscribed) void this.#refreshToken();
      if (this.#supported === false) this.#probeSupport();
    };
    this.#controllerListener = (): void => {
      if (this.#supported === false) this.#probeSupport();
    };
    document.addEventListener("visibilitychange", this.#visibilityListener);
    navigator.serviceWorker?.addEventListener("controllerchange", this.#controllerListener);
  }

  async #wireListeners(): Promise<void> {
    const unsubscribe = await onForegroundMessage((payload) => {
      if (this.#ownerId === null || !this.#subscribed) return;
      this.#lastMessage = publicPushPayload(payload);
      this.#messageSeq += 1;
      void track("notification_received_foreground");
    });
    if (!this.#hydrated) {
      unsubscribe();
      return;
    }
    this.#foregroundUnsubscribe = unsubscribe;
  }

  #current(ownerId: number, generation: number): boolean {
    return this.#ownerId === ownerId && this.#generation === generation;
  }

  async #register(token: string, ownerId: number, generation: number): Promise<boolean> {
    if (!this.#current(ownerId, generation)) return false;
    const controller = new AbortController();
    this.#registerController = controller;
    this.#registeringToken = token;
    const registered = await registerTokenWithServer(token, ownerId, controller.signal);
    if (this.#registerController === controller) {
      this.#registerController = null;
      this.#registeringToken = null;
    }
    if (!this.#current(ownerId, generation)) return false;
    this.#setSubscribed(registered, token);
    this.#pushError = !registered;
    return registered;
  }

  #refreshToken(): Promise<void> {
    if (this.#refreshInFlight) return this.#refreshInFlight;
    const ownerId = this.#ownerId;
    const generation = this.#generation;
    if (ownerId === null) return Promise.resolve();
    const run = (async (): Promise<void> => {
      await this.#teardownInFlight;
      if (!this.#current(ownerId, generation)) return;
      this.#permission = getPermissionState();
      const token = await getFcmToken();
      if (!this.#current(ownerId, generation)) return;
      if (!token) {
        if (this.#permission !== "granted") this.#setSubscribed(false, null);
        return;
      }
      await this.#register(token, ownerId, generation);
    })().catch(() => {
      if (this.#current(ownerId, generation)) this.#pushError = true;
    });
    this.#refreshInFlight = run;
    void run.finally(() => {
      if (this.#refreshInFlight === run) this.#refreshInFlight = null;
    });
    return run;
  }

  #persist(): void {
    if (!browser) return;
    writeJSON(STORAGE_KEY, {
      token: this.#token,
      subscribed: this.#subscribed,
      ownerId: this.#ownerId,
    });
  }

  #setSubscribed(subscribed: boolean, token: string | null): void {
    this.#subscribed = subscribed;
    this.#token = token;
    this.#stored = { token, subscribed, ownerId: this.#ownerId };
    this.#persist();
  }

  async subscribe(): Promise<boolean> {
    const ownerId = this.#ownerId;
    if (!browser || this.#busy || ownerId === null) return false;
    if (!this.#configured || this.#supported === false) return false;
    this.#busy = true;
    this.#pushError = false;
    this.#invalidate();
    const generation = this.#generation;
    try {
      const permission = await requestPermission();
      if (!this.#current(ownerId, generation)) return false;
      this.#permission = permission;
      if (permission !== "granted") return false;
      await this.#teardownInFlight;
      if (!this.#current(ownerId, generation)) return false;
      const token = await getFcmToken();
      if (!this.#current(ownerId, generation)) return false;
      if (!token) {
        this.#pushError = true;
        return false;
      }
      const registered = await this.#register(token, ownerId, generation);
      if (registered) void track("notification_subscribe");
      return registered;
    } catch {
      if (this.#current(ownerId, generation)) this.#pushError = true;
      return false;
    } finally {
      if (this.#current(ownerId, generation)) this.#busy = false;
    }
  }

  async prepareLogout(): Promise<void> {
    const pendingToken = this.#registeringToken;
    this.#invalidate();
    const token = this.#token ?? pendingToken ?? this.#stored?.token ?? null;
    const ownerId = this.#ownerId ?? this.#stored?.ownerId ?? null;
    this.#stored = null;
    this.#subscribed = false;
    this.#token = null;
    this.#lastMessage = null;
    this.#busy = false;
    this.#persist();
    if (token) this.#queueTeardown(token, ownerId);
    await this.#teardownInFlight;
  }

  async unsubscribe(): Promise<boolean> {
    if (!browser || this.#busy) return false;
    await this.prepareLogout();
    this.#pushError = false;
    void track("notification_unsubscribe");
    return true;
  }

  clearMessage(): void {
    this.#lastMessage = null;
  }

  syncPermission(): void {
    if (!browser) return;
    this.#permission = getPermissionState();
    if (this.#permission !== "granted") void this.prepareLogout();
    if (this.#supported === true) void initMessaging();
  }

  dispose(): void {
    this.#invalidate();
    this.#foregroundUnsubscribe?.();
    this.#foregroundUnsubscribe = null;
    if (this.#visibilityListener)
      document.removeEventListener("visibilitychange", this.#visibilityListener);
    if (this.#controllerListener)
      navigator.serviceWorker?.removeEventListener("controllerchange", this.#controllerListener);
    this.#visibilityListener = null;
    this.#controllerListener = null;
    this.#hydrated = false;
  }
}

export function createNotifications(): NotificationsStore {
  return new NotificationsStore();
}
export const notifications = createNotifications();
