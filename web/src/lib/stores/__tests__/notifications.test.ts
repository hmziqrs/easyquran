import type { MessagePayload, Messaging, Unsubscribe } from "firebase/messaging";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { messaging, firebaseCore, analytics } = vi.hoisted(() => ({
  messaging: {
    getPermissionState: vi.fn<() => string>(),
    isMessagingSupported: vi.fn<() => Promise<boolean>>(),
    initMessaging: vi.fn<() => Promise<Messaging | null>>(),
    requestPermission: vi.fn<() => Promise<string>>(),
    getFcmToken: vi.fn<() => Promise<string | null>>(),
    registerTokenWithServer:
      vi.fn<(token: string, ownerId: number, signal?: AbortSignal) => Promise<boolean>>(),
    unregisterTokenFromServer: vi.fn<(token: string, ownerId: number) => Promise<boolean>>(),
    deleteFcmToken: vi.fn<() => Promise<void>>(),
    onForegroundMessage: vi.fn<(cb: (p: MessagePayload) => void) => Promise<Unsubscribe>>(),
  },
  firebaseCore: { isConfigured: true, isMessagingConfigured: true },
  analytics: { track: vi.fn<() => void>() },
}));

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("#lib/firebase/index.js", () => firebaseCore);
vi.mock("#lib/firebase/messaging.js", () => messaging);
vi.mock("#lib/firebase/analytics.js", () => analytics);

import { notificationsStatus } from "#lib/components/notifications/notifications-copy.js";
import { createNotifications } from "#lib/stores/notifications.svelte.js";

const STORAGE_KEY = "easyquran.fcm";
const stores: ReturnType<typeof createNotifications>[] = [];
function createOwnedStore() {
  const store = createNotifications();
  void store.onAuthChanged("authenticated", 7);
  stores.push(store);
  return store;
}
let foregroundCb: ((p: MessagePayload) => void) | undefined;

const statusMessages = {
  unavailable: "unavailable",
  checking: "checking",
  browserUnsupported: "browser-unsupported",
  blocked: "blocked",
  error: "error",
  on: "on",
  offUpdates: "off-updates",
  off: "off",
};

function resetMocks(): void {
  messaging.getPermissionState.mockReset();
  messaging.isMessagingSupported.mockReset();
  messaging.initMessaging.mockReset();
  messaging.requestPermission.mockReset();
  messaging.getFcmToken.mockReset();
  messaging.registerTokenWithServer.mockReset();
  messaging.unregisterTokenFromServer.mockReset();
  messaging.deleteFcmToken.mockReset();
  messaging.onForegroundMessage.mockReset();
  analytics.track.mockReset();
  firebaseCore.isMessagingConfigured = true;
  messaging.getPermissionState.mockReturnValue("granted");
  messaging.isMessagingSupported.mockResolvedValue(true);
  messaging.initMessaging.mockResolvedValue(null);
  messaging.requestPermission.mockResolvedValue("granted");
  messaging.getFcmToken.mockResolvedValue("token-xyz");
  messaging.registerTokenWithServer.mockResolvedValue(true);
  messaging.unregisterTokenFromServer.mockResolvedValue(true);
  messaging.deleteFcmToken.mockResolvedValue(undefined);
  messaging.onForegroundMessage.mockImplementation((cb) => {
    foregroundCb = cb;
    return Promise.resolve(() => {});
  });
  foregroundCb = undefined;
}

async function flush(count = 10): Promise<void> {
  for (let i = 0; i < count; i++) await Promise.resolve();
}

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  resetMocks();
});

afterEach(() => {
  for (const store of stores.splice(0)) store.dispose();
  vi.restoreAllMocks();
});

describe("NotificationsStore.subscribe guards", () => {
  it("rejects anonymous push registration", async () => {
    const store = createNotifications();
    expect(await store.subscribe()).toBe(false);
    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
  });
  it("returns false and skips getFcmToken when subscribe is already busy", async () => {
    const store = createOwnedStore();
    let releasePermission: (value: string) => void = () => {};
    messaging.requestPermission.mockReturnValue(
      new Promise<string>((resolve) => {
        releasePermission = resolve;
      }),
    );
    const first = store.subscribe();
    await flush();
    const second = await store.subscribe();
    expect(second).toBe(false);
    expect(messaging.getFcmToken).not.toHaveBeenCalled();
    releasePermission("granted");
    await first;
  });

  it("returns false without registering when permission is denied", async () => {
    messaging.requestPermission.mockResolvedValue("denied");
    const store = createOwnedStore();
    const result = await store.subscribe();
    expect(result).toBe(false);
    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
    expect(store.subscribed).toBe(false);
  });
});

describe("NotificationsStore hydration", () => {
  it("does not restore an account-owned subscription before authentication resolves", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "old", subscribed: true, ownerId: 7 }),
    );
    const store = createNotifications();
    store.hydrate();
    await flush();
    expect(store.subscribed).toBe(false);
    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
    store.dispose();
  });
  it("skips Messaging listeners and token refresh when unsupported", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "old", subscribed: true, ownerId: 7 }),
    );
    messaging.isMessagingSupported.mockResolvedValue(false);

    const store = createOwnedStore();
    store.hydrate();
    await flush();

    expect(store.supported).toBe(false);
    expect(store.subscribed).toBe(false);
    expect(messaging.onForegroundMessage).not.toHaveBeenCalled();
    expect(messaging.getFcmToken).not.toHaveBeenCalled();
    expect(messaging.initMessaging).not.toHaveBeenCalled();
    store.dispose();
  });

  it("wires once and refreshes an existing subscription when supported", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "old", subscribed: true, ownerId: 7 }),
    );

    const store = createOwnedStore();
    store.hydrate();
    store.hydrate();
    await flush();

    expect(store.supported).toBe(true);
    expect(messaging.onForegroundMessage).toHaveBeenCalledTimes(1);
    expect(messaging.getFcmToken).toHaveBeenCalledTimes(1);
    store.dispose();
  });
});

describe("NotificationsStore.subscribe happy path", () => {
  it("registers the token, flips subscribed, and persists the record", async () => {
    messaging.getFcmToken.mockResolvedValue("abc-123");
    const store = createOwnedStore();
    const result = await store.subscribe();
    expect(result).toBe(true);
    expect(messaging.registerTokenWithServer).toHaveBeenCalledWith(
      "abc-123",
      7,
      expect.any(AbortSignal),
    );
    expect(store.subscribed).toBe(true);
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    expect(stored).toEqual({ token: "abc-123", subscribed: true, ownerId: 7 });
  });
});

describe("NotificationsStore #refreshToken dedup", () => {
  it("shares a single getFcmToken call across concurrent refresh triggers", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "old", subscribed: true, ownerId: 7 }),
    );
    let release: (value: string | null) => void = () => {};
    messaging.getFcmToken.mockReturnValue(
      new Promise<string | null>((resolve) => {
        release = resolve;
      }),
    );
    const store = createOwnedStore();
    store.hydrate();
    await flush();
    expect(messaging.getFcmToken).toHaveBeenCalledTimes(1);
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(messaging.getFcmToken).toHaveBeenCalledTimes(1);
    release("new-token");
    await flush();
    store.dispose();
  });
});

describe("NotificationsStore generation race-guard", () => {
  it("does not register the stale token after unsubscribe bumps the generation", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "old", subscribed: true, ownerId: 7 }),
    );
    let release: (value: string | null) => void = () => {};
    messaging.getFcmToken.mockReturnValue(
      new Promise<string | null>((resolve) => {
        release = resolve;
      }),
    );
    messaging.unregisterTokenFromServer.mockResolvedValue(true);
    messaging.deleteFcmToken.mockResolvedValue(undefined);

    const store = createOwnedStore();
    store.hydrate();
    await flush();

    expect(messaging.getFcmToken).toHaveBeenCalledTimes(1);
    const unsub = store.unsubscribe();
    await flush();
    release("new-token");
    await unsub;
    await flush();

    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
    expect(store.subscribed).toBe(false);
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    expect(stored).toEqual({ token: null, subscribed: false, ownerId: 7 });
    store.dispose();
  });
});

describe("NotificationsStore foreground message", () => {
  it("exposes the foreground payload and bumps the sequence counter", async () => {
    const store = createOwnedStore();
    await store.subscribe();
    store.hydrate();
    await flush();
    expect(foregroundCb).toBeTypeOf("function");
    const before = store.messageSeq;
    // SAFETY: minimal fixture for the foreground-message path; only messageId (a required string field) is read.
    foregroundCb?.({ messageId: "msg-1" } as MessagePayload);
    expect(store.lastMessage).toMatchObject({ messageId: "msg-1" });
    expect(store.messageSeq).toBe(before + 1);
    store.dispose();
  });

  it("clearMessage nulls the last message", async () => {
    const store = createOwnedStore();
    await store.subscribe();
    store.hydrate();
    await flush();
    // SAFETY: minimal fixture for the foreground-message path; only messageId (a required string field) is read.
    foregroundCb?.({ messageId: "msg-2" } as MessagePayload);
    expect(store.lastMessage).not.toBeNull();
    store.clearMessage();
    expect(store.lastMessage).toBeNull();
    store.dispose();
  });
});

describe("NotificationsStore.syncPermission", () => {
  it("revokes subscription when permission has flipped to denied", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "tok", subscribed: true, ownerId: 7 }),
    );
    const store = createOwnedStore();
    store.hydrate();
    await flush();
    expect(store.subscribed).toBe(true);

    messaging.getPermissionState.mockReturnValue("denied");
    store.syncPermission();

    expect(store.subscribed).toBe(false);
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    expect(stored).toEqual({ token: null, subscribed: false, ownerId: 7 });
    store.dispose();
  });
});

describe("NotificationsStore capability split", () => {
  it("marks the store configured and supported on the Chrome-capable path", async () => {
    const store = createOwnedStore();
    expect(store.supported).toBe(null);
    store.hydrate();
    await flush();
    expect(store.configured).toBe(true);
    expect(store.supported).toBe(true);
  });

  it("reports unavailable when messaging is not configured even though the probe passes", async () => {
    firebaseCore.isMessagingConfigured = false;
    const store = createOwnedStore();
    store.hydrate();
    await flush();
    expect(store.supported).toBe(true);
    expect(store.configured).toBe(false);
    const label = notificationsStatus(statusMessages)({
      configured: store.configured,
      supported: store.supported,
      permission: store.permission,
      subscribed: store.subscribed,
      pushError: store.pushError,
    });
    expect(label).toBe("unavailable");
  });

  it("re-runs the support probe on visible tab return while unsupported", async () => {
    messaging.isMessagingSupported.mockResolvedValueOnce(false).mockResolvedValue(true);
    const store = createOwnedStore();
    store.hydrate();
    await flush();
    expect(store.supported).toBe(false);
    document.dispatchEvent(new Event("visibilitychange"));
    await flush();
    expect(store.supported).toBe(true);
  });

  it("flags pushError when the token fetch fails and clears it on a later success", async () => {
    messaging.getFcmToken.mockResolvedValue(null);
    const store = createOwnedStore();
    await expect(store.subscribe()).resolves.toBe(false);
    expect(store.pushError).toBe(true);
    expect(
      notificationsStatus(statusMessages)({
        configured: true,
        supported: true,
        permission: store.permission,
        subscribed: store.subscribed,
        pushError: store.pushError,
      }),
    ).toBe("error");

    messaging.getFcmToken.mockResolvedValue("tok-retry");
    await expect(store.subscribe()).resolves.toBe(true);
    expect(store.pushError).toBe(false);
  });
});

describe("NotificationsStore account privacy", () => {
  it("always registers unchanged tokens for the authenticated owner", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "token-xyz", subscribed: true, ownerId: 7 }),
    );
    const store = createOwnedStore();
    store.hydrate();
    await flush(30);
    expect(messaging.registerTokenWithServer).toHaveBeenCalledWith(
      "token-xyz",
      7,
      expect.any(AbortSignal),
    );
    expect(store.subscribed).toBe(true);
  });

  it("clears former owner and tears down registration before another account subscribes", async () => {
    const store = createOwnedStore();
    await store.subscribe();
    await store.onAuthChanged("authenticated", 8);
    expect(store.subscribed).toBe(false);
    expect(messaging.unregisterTokenFromServer).toHaveBeenCalledWith("token-xyz", 7);
    expect(messaging.deleteFcmToken).toHaveBeenCalledOnce();
    await store.subscribe();
    expect(messaging.registerTokenWithServer).toHaveBeenLastCalledWith(
      "token-xyz",
      8,
      expect.any(AbortSignal),
    );
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null")).toEqual({
      token: "token-xyz",
      subscribed: true,
      ownerId: 8,
    });
  });

  it("aborts and ignores an old account registration completing after a switch", async () => {
    let release: (value: boolean) => void = () => {};
    messaging.registerTokenWithServer.mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        release = resolve;
      }),
    );
    const store = createOwnedStore();
    const subscription = store.subscribe();
    await flush(30);
    const signal = messaging.registerTokenWithServer.mock.calls[0]?.[2];
    await store.onAuthChanged("authenticated", 8);
    expect(signal?.aborted).toBe(true);
    release(true);
    expect(await subscription).toBe(false);
    expect(store.subscribed).toBe(false);
    await store.subscribe();
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null")?.ownerId).toBe(8);
  });

  it("rejects delayed token results from the former owner", async () => {
    let release: (value: string | null) => void = () => {};
    messaging.getFcmToken.mockReturnValueOnce(
      new Promise<string | null>((resolve) => {
        release = resolve;
      }),
    );
    const store = createOwnedStore();
    const subscription = store.subscribe();
    await flush(30);
    await store.onAuthChanged("authenticated", 8);
    release("former-owner-token");
    expect(await subscription).toBe(false);
    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
  });

  it("rejects delayed permission results after logout", async () => {
    let release: (value: string) => void = () => {};
    messaging.requestPermission.mockReturnValueOnce(
      new Promise<string>((resolve) => {
        release = resolve;
      }),
    );
    const store = createOwnedStore();
    const subscription = store.subscribe();
    await store.prepareLogout();
    release("granted");
    expect(await subscription).toBe(false);
    expect(messaging.getFcmToken).not.toHaveBeenCalled();
  });

  it("revokes foreign or ownerless persisted tokens when booting under a known account", async () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ token: "former-token", subscribed: true, ownerId: 8 }),
    );
    const store = createOwnedStore();
    store.hydrate();
    await flush(30);
    expect(store.subscribed).toBe(false);
    expect(messaging.unregisterTokenFromServer).toHaveBeenCalledWith("former-token", 8);
    expect(messaging.deleteFcmToken).toHaveBeenCalledOnce();
    expect(messaging.registerTokenWithServer).not.toHaveBeenCalled();
  });

  it("clears locally on offline logout even when server and Firebase cleanup fail", async () => {
    const store = createOwnedStore();
    await store.subscribe();
    messaging.unregisterTokenFromServer.mockRejectedValue(new Error("offline"));
    messaging.deleteFcmToken.mockRejectedValue(new Error("offline"));
    await expect(store.prepareLogout()).resolves.toBeUndefined();
    expect(store.subscribed).toBe(false);
    expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null")?.token).toBeNull();
  });

  it("never displays private legacy foreground content or identifiers", async () => {
    const store = createOwnedStore();
    store.hydrate();
    await flush(30);
    await store.subscribe();
    foregroundCb?.({
      from: "fcm",
      collapseKey: "private-123",
      messageId: "m",
      notification: { title: "Secret title", body: "Private body" },
      data: { url: "/private", user_id: "7", body: "More private content" },
    });
    expect(store.lastMessage?.notification).toEqual({
      title: "EasyQuran",
      body: "Open EasyQuran to view notifications.",
    });
    expect(store.lastMessage?.data).toBeUndefined();
    await store.onAuthChanged("anonymous", null);
    foregroundCb?.({ from: "fcm", collapseKey: "private-123", messageId: "m" });
    expect(store.lastMessage).toBeNull();
  });
});
