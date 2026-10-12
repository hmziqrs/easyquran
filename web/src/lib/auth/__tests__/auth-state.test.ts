import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { envMock, clientMock } = vi.hoisted(() => ({
  envMock: { browser: true },
  clientMock: {
    getUser: vi.fn<() => Promise<SessionProbeResult>>(),
    clearCsrf: vi.fn(),
  },
}));

vi.mock("$app/env", () => ({
  get browser(): boolean {
    return envMock.browser;
  },
}));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));
vi.mock("#lib/auth/auth-client.js", () => ({
  authClient: clientMock,
  createAuthClient: () => clientMock,
}));

import type { SessionProbeResult } from "#lib/auth/auth-client.js";
import type { AuthTransitionContext } from "#lib/auth/auth-state.svelte.js";
import {
  SESSION_PROBE_INTERVAL_MS,
  createAuthState as createAuthStateBase,
} from "#lib/auth/auth-state.svelte.js";

// Every state created in this file is disposed after its test: hydrated states
// keep a window `storage` listener, and cross-tab tests dispatch real events.
const created: Array<{ dispose(): void }> = [];
function createAuthState(...args: Parameters<typeof createAuthStateBase>) {
  const state = createAuthStateBase(...args);
  created.push(state);
  return state;
}

// SAFETY: createAuthState only invokes getUser() on the client; the hoisted double supplies it as a vi.fn() resolving SessionProbeResult.
const asClient = clientMock as never;

const PROFILE = {
  id: 7,
  name: "Sara",
  email: "sara@eq.test",
  avatar_id: null,
  is_verified: true,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: null,
};

const HINT_KEY = "eq.auth.session-hint";

beforeEach(() => {
  envMock.browser = true;
  clientMock.getUser.mockReset();
  clientMock.clearCsrf.mockReset();
  localStorage.clear();
});

afterEach(() => {
  for (const state of created) state.dispose();
  created.length = 0;
  vi.restoreAllMocks();
});

describe("AuthState.probe single-flight", () => {
  it("concurrent probe calls share a single getUser request", async () => {
    let resolveProbe!: (v: SessionProbeResult) => void;
    clientMock.getUser.mockReturnValue(
      new Promise<SessionProbeResult>((res) => {
        resolveProbe = res;
      }),
    );
    const state = createAuthState(asClient);
    const p1 = state.probe();
    const p2 = state.probe();
    const p3 = state.probe();
    expect(clientMock.getUser).toHaveBeenCalledTimes(1);
    resolveProbe({ kind: "authenticated", user: PROFILE });
    const [r1, r2, r3] = await Promise.all([p1, p2, p3]);
    expect(r1).toEqual({ kind: "authenticated", user: PROFILE });
    expect(r2).toBe(r1);
    expect(r3).toBe(r1);
  });

  it("after a probe settles, the next call issues a fresh getUser", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
    const state = createAuthState(asClient);
    await state.probe();
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    await state.probe();
    expect(clientMock.getUser).toHaveBeenCalledTimes(2);
  });
});

describe("AuthState hydrate after mount", () => {
  it("issues a probe when browser=true and the session hint is present", async () => {
    envMock.browser = true;
    localStorage.setItem(HINT_KEY, "1");
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    const state = createAuthState(asClient);
    state.hydrate();
    expect(state.hydrated).toBe(true);
    await state.probe();
    expect(clientMock.getUser).toHaveBeenCalledTimes(1);
    expect(state.status).toBe("anonymous");
  });

  it("is a no-op during SSR (browser=false): no probe, stays unknown", () => {
    envMock.browser = false;
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    const state = createAuthState(asClient);
    state.hydrate();
    expect(state.hydrated).toBe(false);
    expect(clientMock.getUser).not.toHaveBeenCalled();
    expect(state.status).toBe("unknown");
    envMock.browser = true;
  });

  it("hydrate is idempotent: a second call does not re-probe", async () => {
    localStorage.setItem(HINT_KEY, "1");
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    const state = createAuthState(asClient);
    state.hydrate();
    state.hydrate();
    await state.probe();
    expect(state.hydrated).toBe(true);
  });
});

describe("AuthState session hint gates the hydrate probe", () => {
  it("skips the probe and settles anonymous when no hint exists", () => {
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    const state = createAuthState(asClient);
    state.hydrate();
    expect(clientMock.getUser).not.toHaveBeenCalled();
    expect(state.status).toBe("anonymous");
    expect(state.hydrated).toBe(true);
  });

  it("force:true probes even without a hint", async () => {
    clientMock.getUser.mockResolvedValue({ kind: "authenticated", user: PROFILE });
    const state = createAuthState(asClient);
    state.hydrate({ force: true });
    await state.probe();
    expect(clientMock.getUser).toHaveBeenCalledTimes(1);
    expect(state.status).toBe("authenticated");
  });

  it("an authenticated probe writes the hint so the next load probes again", async () => {
    clientMock.getUser.mockResolvedValue({ kind: "authenticated", user: PROFILE });
    await createAuthState(asClient).probe();
    expect(localStorage.getItem(HINT_KEY)).not.toBeNull();

    clientMock.getUser.mockClear();
    const next = createAuthState(asClient);
    next.hydrate();
    expect(clientMock.getUser).toHaveBeenCalledTimes(1);
  });

  it("a stale hint self-heals: an anonymous probe clears it", async () => {
    localStorage.setItem(HINT_KEY, "1");
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    await createAuthState(asClient).probe();
    expect(localStorage.getItem(HINT_KEY)).toBeNull();
  });

  it("an unknown probe leaves the hint untouched (transport failure is not a logout)", async () => {
    localStorage.setItem(HINT_KEY, "1");
    clientMock.getUser.mockResolvedValue({ kind: "unknown" });
    await createAuthState(asClient).probe();
    expect(localStorage.getItem(HINT_KEY)).toBe("1");
  });

  it("setUser writes the hint on login and clears it on sign-out", () => {
    const state = createAuthState(asClient);
    state.setUser(PROFILE);
    expect(localStorage.getItem(HINT_KEY)).not.toBeNull();
    state.setUser(null);
    expect(localStorage.getItem(HINT_KEY)).toBeNull();
  });

  it("every sign-in mints a fresh nonce so account switches fire storage events (EC-31)", () => {
    const state = createAuthState(asClient);
    state.setUser(PROFILE);
    const first = localStorage.getItem(HINT_KEY);
    expect(first).not.toBeNull();
    state.setUser(null);
    state.setUser({ ...PROFILE, id: 9 });
    const second = localStorage.getItem(HINT_KEY);
    expect(second).not.toBeNull();
    expect(second).not.toBe(first);
  });

  it("reset (logout) clears the hint", () => {
    localStorage.setItem(HINT_KEY, "1");
    const state = createAuthState(asClient);
    state.reset();
    expect(localStorage.getItem(HINT_KEY)).toBeNull();
  });
});

describe("AuthState probe result mapping", () => {
  it("200 authenticates and stores the profile", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    const state = createAuthState(asClient);
    await state.probe();
    expect(state.status).toBe("authenticated");
    expect(state.authenticated).toBe(true);
    expect(state.user?.id).toBe(7);
    expect(state.anonymous).toBe(false);
    expect(state.unknown).toBe(false);
  });

  it("401/403 maps to anonymous and clears any user", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
    const state = createAuthState(asClient);
    await state.probe();
    expect(state.authenticated).toBe(true);
    await state.probe();
    expect(state.status).toBe("anonymous");
    expect(state.anonymous).toBe(true);
    expect(state.user).toBeNull();
  });

  it("transport failure stays retryable unknown", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "unknown" });
    const state = createAuthState(asClient);
    await state.probe();
    expect(state.status).toBe("unknown");
    expect(state.unknown).toBe(true);
    expect(state.user).toBeNull();
  });

  it("5xx stays retryable unknown with status preserved", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "unknown", status: 503 });
    const state = createAuthState(asClient);
    const result = await state.probe();
    expect(state.status).toBe("unknown");
    expect(result).toEqual({ kind: "unknown", status: 503 });
  });
});

describe("AuthState onAuthTransition hook (W8a purgeUserCaches placeholder)", () => {
  it("invokes the registered hook during transition()", async () => {
    const hook = vi
      .fn<(ctx: AuthTransitionContext) => Promise<void>>()
      .mockResolvedValue(undefined);
    const state = createAuthState(asClient);
    state.setOnAuthTransition(hook);
    await state.transition({ kind: "login" });
    expect(hook).toHaveBeenCalledWith({ kind: "login" });
  });

  it("transition resolves when no hook is registered", async () => {
    const state = createAuthState(asClient);
    await expect(state.transition({ kind: "logout" })).resolves.toBeUndefined();
  });

  it("awaiting the hook means purge completes before the transition returns", async () => {
    const order: string[] = [];
    let resolvePurge!: () => void;
    const hook = vi.fn(
      () =>
        new Promise<void>((res) => {
          resolvePurge = res;
        }),
    );
    const state = createAuthState(asClient);
    state.setOnAuthTransition(hook);
    const p = state.transition({ kind: "oauth" });
    let afterTransition = false;
    void p.then(() => {
      order.push("transition-resolved");
      afterTransition = true;
    });
    await Promise.resolve();
    expect(hook).toHaveBeenCalled();
    expect(afterTransition).toBe(false);
    order.push("purge-resolved");
    resolvePurge();
    await p;
    expect(order).toEqual(["purge-resolved", "transition-resolved"]);
  });

  it("hook can be cleared by passing null", async () => {
    const hook = vi.fn().mockResolvedValue(undefined);
    const state = createAuthState(asClient);
    state.setOnAuthTransition(hook);
    state.setOnAuthTransition(null);
    await state.transition({ kind: "two-fa-verify" });
    expect(hook).not.toHaveBeenCalled();
  });
});

describe("AuthState manual state setters", () => {
  it("setUser toggles authenticated/anonymous", () => {
    const state = createAuthState(asClient);
    state.setUser(PROFILE);
    expect(state.authenticated).toBe(true);
    state.setUser(null);
    expect(state.anonymous).toBe(true);
    expect(state.user).toBeNull();
  });

  it("setUnknown and reset return to the unknown state", () => {
    const state = createAuthState(asClient);
    state.setUser(PROFILE);
    state.setUnknown();
    expect(state.unknown).toBe(true);
    expect(state.user).toBeNull();
    state.reset();
    expect(state.hydrated).toBe(false);
  });
});

/** Let the fire-and-forget terminate/probe continuations settle. */
async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("AuthState session loss mid-use (EC-15/16/19)", () => {
  it("an anonymous probe from an authenticated view fires current-session-terminated + banner", async () => {
    const hook = vi.fn().mockResolvedValue(undefined);
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
    const state = createAuthState(asClient);
    await state.probe();
    state.setOnAuthTransition(hook);
    await state.probe();
    expect(state.status).toBe("anonymous");
    expect(state.sessionExpired).toBe(true);
    expect(clientMock.clearCsrf).toHaveBeenCalledTimes(1);
    expect(localStorage.getItem(HINT_KEY)).toBeNull();
    expect(hook).toHaveBeenCalledWith({ kind: "current-session-terminated" });
  });

  it("a transport-failure probe never terminates the signed-in view", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    clientMock.getUser.mockResolvedValueOnce({ kind: "unknown", status: 503 });
    const state = createAuthState(asClient);
    await state.probe();
    await state.probe();
    expect(state.status).toBe("authenticated");
    expect(state.sessionExpired).toBe(false);
    expect(clientMock.clearCsrf).not.toHaveBeenCalled();
  });

  it("a cold-boot anonymous probe (no prior authenticated view) does not terminate", async () => {
    const hook = vi.fn().mockResolvedValue(undefined);
    clientMock.getUser.mockResolvedValue({ kind: "anonymous" });
    const state = createAuthState(asClient);
    state.setOnAuthTransition(hook);
    await state.probe();
    expect(state.status).toBe("anonymous");
    expect(state.sessionExpired).toBe(false);
    expect(hook).not.toHaveBeenCalled();
  });

  it("the banner flag clears on the next successful sign-in", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
    const state = createAuthState(asClient);
    await state.probe();
    await state.probe();
    expect(state.sessionExpired).toBe(true);
    state.setUser(PROFILE);
    expect(state.sessionExpired).toBe(false);
  });

  it("notifySessionLost clears CSRF and lets the probe decide", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
    const state = createAuthState(asClient);
    await state.probe();
    clientMock.getUser.mockClear();
    state.notifySessionLost();
    expect(clientMock.clearCsrf).toHaveBeenCalledTimes(1);
    await settle();
    expect(clientMock.getUser).toHaveBeenCalledTimes(1);
    expect(state.status).toBe("anonymous");
    expect(state.sessionExpired).toBe(true);
  });
});

describe("AuthState revocation watchdog (EC-15 idle tabs)", () => {
  it("re-probes on the interval while authenticated; an anonymous result terminates with the banner", async () => {
    vi.useFakeTimers();
    try {
      clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
      clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
      const hook = vi.fn().mockResolvedValue(undefined);
      const state = createAuthState(asClient);
      state.setOnAuthTransition(hook);
      await state.probe();
      expect(state.authenticated).toBe(true);
      expect(clientMock.getUser).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);

      expect(clientMock.getUser).toHaveBeenCalledTimes(2);
      expect(state.status).toBe("anonymous");
      expect(state.sessionExpired).toBe(true);
      expect(hook).toHaveBeenCalledWith({ kind: "current-session-terminated" });
    } finally {
      vi.useRealTimers();
    }
  });

  it("stops probing once anonymous: later ticks issue no requests", async () => {
    vi.useFakeTimers();
    try {
      clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
      clientMock.getUser.mockResolvedValueOnce({ kind: "anonymous" });
      const state = createAuthState(asClient);
      await state.probe();
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(clientMock.getUser).toHaveBeenCalledTimes(2);
      expect(state.anonymous).toBe(true);

      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS * 3);
      expect(clientMock.getUser).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("setUser(null) and dispose() tear the watchdog down", async () => {
    vi.useFakeTimers();
    try {
      const state = createAuthState(asClient);
      state.setUser(PROFILE);
      state.setUser(null);
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(clientMock.getUser).not.toHaveBeenCalled();

      state.setUser(PROFILE);
      state.dispose();
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(clientMock.getUser).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("never schedules during SSR (browser=false)", async () => {
    vi.useFakeTimers();
    envMock.browser = false;
    try {
      const state = createAuthState(asClient);
      state.setUser(PROFILE);
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(clientMock.getUser).not.toHaveBeenCalled();
    } finally {
      envMock.browser = true;
      vi.useRealTimers();
    }
  });

  it("a transport-failure watchdog probe keeps the session and the timer alive", async () => {
    vi.useFakeTimers();
    try {
      clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
      clientMock.getUser.mockResolvedValueOnce({ kind: "unknown", status: 503 });
      clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
      const state = createAuthState(asClient);
      await state.probe();
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(state.authenticated).toBe(true);
      expect(state.sessionExpired).toBe(false);
      await vi.advanceTimersByTimeAsync(SESSION_PROBE_INTERVAL_MS);
      expect(clientMock.getUser).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("AuthState cross-tab session hint (EC-31)", () => {
  function hintEvent(newValue: string | null): StorageEvent {
    return new StorageEvent("storage", { key: HINT_KEY, newValue });
  }

  it("a hint removal in another tab terminates this tab without the expiry banner", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    const hook = vi.fn().mockResolvedValue(undefined);
    const state = createAuthState(asClient);
    state.hydrate({ force: true });
    await settle();
    expect(state.status).toBe("authenticated");
    state.setOnAuthTransition(hook);
    window.dispatchEvent(hintEvent(null));
    await settle();
    expect(state.status).toBe("anonymous");
    expect(state.sessionExpired).toBe(false);
    expect(clientMock.clearCsrf).toHaveBeenCalledTimes(1);
    expect(hook).toHaveBeenCalledWith({ kind: "current-session-terminated" });
  });

  it("a hint change in another tab re-probes and adopts the new account", async () => {
    clientMock.getUser.mockResolvedValueOnce({ kind: "authenticated", user: PROFILE });
    const hook = vi.fn().mockResolvedValue(undefined);
    const state = createAuthState(asClient);
    state.hydrate({ force: true });
    await settle();
    state.setOnAuthTransition(hook);
    // mockResolvedValue (not Once): earlier tests' hydrated states also probe on
    // this event; every probe must see the same new account.
    const otherUser = { ...PROFILE, id: 9 };
    clientMock.getUser.mockResolvedValue({ kind: "authenticated", user: otherUser });
    window.dispatchEvent(hintEvent("nonce-from-other-tab"));
    await settle();
    expect(state.status).toBe("authenticated");
    expect(state.user?.id).toBe(9);
    expect(hook).toHaveBeenCalledWith({ kind: "login" });
  });
});
