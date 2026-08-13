import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { envMock, clientMock } = vi.hoisted(() => ({
  envMock: { browser: true },
  clientMock: {
    getUser: vi.fn<() => Promise<unknown>>(),
  },
}));

vi.mock("$app/environment", () => ({
  get browser(): boolean {
    return envMock.browser;
  },
}));
vi.mock("$lib/auth/auth-client", () => ({
  authClient: clientMock,
  createAuthClient: () => clientMock,
}));

import type { AuthClient } from "$lib/auth/auth-client";
import { createAuthState } from "$lib/auth/auth-state.svelte";

const asClient = clientMock as unknown as AuthClient;

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
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("AuthState.probe single-flight", () => {
  it("concurrent probe calls share a single getUser request", async () => {
    let resolveProbe!: (v: unknown) => void;
    clientMock.getUser.mockReturnValue(
      new Promise((res) => {
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
    expect(localStorage.getItem(HINT_KEY)).toBe("1");

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
    expect(localStorage.getItem(HINT_KEY)).toBe("1");
    state.setUser(null);
    expect(localStorage.getItem(HINT_KEY)).toBeNull();
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
    const hook = vi.fn<(ctx: unknown) => Promise<void>>().mockResolvedValue(undefined);
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
