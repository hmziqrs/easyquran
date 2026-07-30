/* ════════════════════════════════════════════════════════════════════════
   notifications.svelte.ts — the push-notification experience state.

   A single Svelte 5 runes class, SSR-safe. It orchestrates the full FCM
   lifecycle on top of lib/firebase/messaging (the thin client):
     • hydrate()   — detect support + permission; if the user previously enabled
                     notifications, re-establish the token and listeners.
     • subscribe() — the user-gesture flow: ask permission → get token → keep it
                     locally → best-effort register with the backend → log event.
     • unsubscribe()— revoke server-side, invalidate the FCM token, clear state.

   The token is persisted to localStorage so it survives reloads, and so a token
   obtained before accounts shipped can be registered with the backend on a later
   login. Foreground messages are mirrored into `lastMessage` for the toast.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { MessagePayload } from "firebase/messaging";
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

const STORAGE_KEY = "easyquran.fcm";

interface PersistedFcm {
  token: string | null;
  subscribed: boolean;
}

function load(): PersistedFcm {
  if (!browser) return { token: null, subscribed: false };
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return {
      token: typeof stored.token === "string" ? stored.token : null,
      subscribed: Boolean(stored.subscribed),
    };
  } catch {
    return { token: null, subscribed: false };
  }
}

class NotificationsStore {
  #permission = $state<PermissionState>("default");
  #supported = $state<boolean | null>(null); // null = not yet probed
  #subscribed = $state(false);
  #token = $state<string | null>(null);
  #lastMessage = $state<MessagePayload | null>(null);
  /** True once a subscribe/unsubscribe is in flight. */
  #busy = $state(false);
  #hydrated = false;
  /**
   * Monotonic generation. Bumped at the start of every subscribe()/unsubscribe()
   * so an in-flight #refreshToken can detect that the subscription state changed
   * under it and bail before writing a stale "subscribed" result.
   */
  #generation = 0;
  /** Coalesces concurrent #refreshToken calls (hydrate + visibilitychange). */
  #refreshInFlight: Promise<void> | null = null;

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
  get canSubscribe(): boolean {
    // Need a definitive "yes" on support before offering the control.
    return this.#supported === true && this.#permission !== "denied";
  }
  /** Human-friendly status line for the settings UI. */
  get statusText(): string {
    if (!isConfigured) return "Notifications unavailable (not configured).";
    if (this.#supported === null) return "Checking…";
    if (this.#supported === false) return "Not supported on this browser.";
    if (this.#permission === "denied") return "Blocked in your browser settings.";
    if (this.#subscribed) return "On — you'll receive notifications.";
    if (this.#permission === "default") return "Off — enable to get updates.";
    return "Off.";
  }

  /** Detect support/permission and, if previously enabled, re-arm push. */
  hydrate(): void {
    if (this.#hydrated || !browser) return;
    this.#hydrated = true;

    const stored = load();
    this.#token = stored.token;
    this.#subscribed = stored.subscribed;
    this.#permission = getPermissionState();

    // If the user revoked permission at the OS/browser level, stop claiming we're subscribed.
    if (this.#permission !== "granted") this.#subscribed = false;

    void isMessagingSupported().then((ok) => {
      this.#supported = ok;
      // Unsupported? Make sure we don't advertise an active subscription.
      if (!ok) this.#subscribed = false;
    });

    // Attach foreground delivery + token-refresh handling for the whole session.
    void this.#wireListeners();

    // Re-establish a valid token + server registration if the user is opted in.
    if (this.#subscribed && this.#permission === "granted") void this.#refreshToken();
  }

  async #wireListeners(): Promise<void> {
    if (!browser) return;
    await onForegroundMessage((payload) => {
      this.#lastMessage = payload;
      void track("notification_received_foreground", { message_id: payload.messageId });
    });
    // The modular messaging SDK exposes no token-refresh callback. The supported
    // way to catch a refresh is to re-call getToken() on app focus and compare —
    // re-register with the backend only when the token actually changed.
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible" && this.#subscribed) {
        void this.#refreshToken();
      }
    });
  }

  /**
   * Re-acquire the current token and re-register it when it changed. Coalesces
   * concurrent calls (hydrate + repeated visibilitychange) and is serialized
   * against subscribe()/unsubscribe() via the generation counter — so a refresh
   * that was in flight when the user opted out can never flip the store back to
   * subscribed or re-register a token after teardown.
   */
  #refreshToken(): Promise<void> {
    if (this.#refreshInFlight) return this.#refreshInFlight;
    const run = (async (): Promise<void> => {
      // Re-sync the live permission — it can change while the tab is hidden, and
      // the cached field drives both the subscription flag and statusText.
      this.#permission = getPermissionState();
      const gen = this.#generation;
      const token = await getFcmToken();
      if (gen !== this.#generation) return; // subscribe/unsubscribe landed mid-flight
      if (!token) {
        // Permission revoked or token unavailable — give up the local subscription.
        if (this.#permission !== "granted") this.#setSubscribed(false, null);
        return;
      }
      if (token === this.#token) {
        // Unchanged: keep the flag/local state without a redundant server round-trip.
        this.#setSubscribed(true, token);
        return;
      }
      const registered = await registerTokenWithServer(token);
      if (gen !== this.#generation) return; // state changed while registering
      this.#token = token;
      // Keep locally regardless; backend may be absent or the session not yet authed.
      this.#setSubscribed(true, token, registered);
      void track("notification_token_refresh");
    })();
    this.#refreshInFlight = run;
    void run.finally(() => {
      this.#refreshInFlight = null;
    });
    return run;
  }

  #persist(): void {
    if (!browser) return;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ token: this.#token, subscribed: this.#subscribed }),
      );
    } catch {
      /* storage may be unavailable (private mode, quota) — non-fatal */
    }
  }

  #setSubscribed(subscribed: boolean, token: string | null, registered = true): void {
    this.#subscribed = subscribed;
    this.#token = token;
    this.#persist();
    if (!registered && token) {
      console.info("[firebase] token kept locally; backend registration pending auth/config.");
    }
  }

  /**
   * The opt-in flow. MUST be called from a user gesture (Safari/iOS web push
   * requires it). Returns true if the device is now subscribed.
   */
  async subscribe(): Promise<boolean> {
    if (!browser || this.#busy) return false;
    if (this.#supported === false) return false;
    this.#busy = true;
    this.#generation += 1; // invalidate any in-flight #refreshToken
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

  /** Opt out: revoke server-side, invalidate the token, clear local state. */
  async unsubscribe(): Promise<boolean> {
    if (!browser || this.#busy) return false;
    this.#busy = true;
    this.#generation += 1; // invalidate any in-flight #refreshToken
    // Let an in-flight refresh finish (its gen check now fails, so it won't write)
    // before we invalidate the token — avoids interleaving getToken/deleteToken.
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

  /** Drop the last foreground message (toast dismissed). */
  clearMessage(): void {
    this.#lastMessage = null;
  }

  /** Re-read the live permission (e.g. after the user returns from settings). */
  syncPermission(): void {
    if (!browser) return;
    this.#permission = getPermissionState();
    if (this.#permission !== "granted") {
      this.#setSubscribed(false, null);
    }
    void initMessaging();
  }
}

export const notifications = new NotificationsStore();
