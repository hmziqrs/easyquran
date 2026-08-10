/* eslint-disable @typescript-eslint/unbound-method */
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import { createOAuthFlow, OAUTH_START_FAILED, type OAuthFlowStateLike, type OAuthProvider } from "$lib/auth/oauth-flow.svelte";
import type { AuthClient, AuthRequestResult, UserProfile } from "$lib/auth/auth-client";
import { consumeReturnTarget, getReturnTarget } from "$lib/auth/return-target";

const PROFILE: UserProfile = {
  id: 9,
  name: "OAuth U",
  email: "oauth@eq.test",
  avatar_id: null,
  is_verified: true,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: "google",
};

function mockClient() {
  return {
    apiBase: "https://eq.test/api",
    unsafeRequest: vi.fn(),
    ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
    get: vi.fn(),
  } as unknown as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    ensureAnonymousSession: ReturnType<typeof vi.fn>;
    refreshCsrf: ReturnType<typeof vi.fn>;
  };
}

type OAuthStateMock = OAuthFlowStateLike & {
  transition: ReturnType<typeof vi.fn>;
  setUser: ReturnType<typeof vi.fn>;
  probe: ReturnType<typeof vi.fn>;
};

function mockState(): OAuthStateMock {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "authenticated", user: PROFILE }),
  } as unknown as OAuthStateMock;
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}

function stateOf<T extends { mock: { invocationCallOrder: number[] } }>(fn: T): number {
  return fn.mock.invocationCallOrder[0] ?? 0;
}

afterEach(() => {
  sessionStorage.clear();
  vi.restoreAllMocks();
});

describe("OAuthFlow.begin bootstrap-before-navigate", () => {
  it("calls ensureAnonymousSession BEFORE navigating to /api/auth/{provider}/v1/login", async () => {
    const client = mockClient();
    const navigate = vi.fn();
    const flow = createOAuthFlow("google", { client, state: mockState(), navigate });
    const res = await flow.begin("/account");

    expect(res).toBe(true);
    expect(navigate).toHaveBeenCalledTimes(1);
    expect(navigate).toHaveBeenCalledWith("https://eq.test/api/auth/google/v1/login");
    const bootOrder = stateOf(
      client.ensureAnonymousSession as unknown as { mock: { invocationCallOrder: number[] } },
    );
    const navOrder = (navigate.mock.invocationCallOrder as number[])[0] ?? 0;
    expect(bootOrder).toBeGreaterThan(0);
    expect(bootOrder).toBeLessThan(navOrder);
  });

  it("stores the return target in sessionStorage before navigating", async () => {
    const client = mockClient();
    const navigate = vi.fn();
    const flow = createOAuthFlow("github", { client, state: mockState(), navigate });
    await flow.begin("/reader/2");
    expect(getReturnTarget()).toBe("/reader/2");
  });

  it("omits a return target when none provided", async () => {
    const client = mockClient();
    const navigate = vi.fn();
    const flow = createOAuthFlow("apple", { client, state: mockState(), navigate });
    await flow.begin();
    expect(getReturnTarget()).toBeNull();
    expect(navigate).toHaveBeenCalledWith("https://eq.test/api/auth/apple/v1/login");
  });

  it("bootstrap failure blocks navigation and sets opaque error code", async () => {
    const client = mockClient();
    client.ensureAnonymousSession.mockRejectedValueOnce(new Error("csrf down"));
    const navigate = vi.fn();
    const flow = createOAuthFlow("facebook", { client, state: mockState(), navigate });
    const res = await flow.begin("/x");
    expect(res).toBe(false);
    expect(navigate).not.toHaveBeenCalled();
    expect(flow.lastErrorCode).toBe(OAUTH_START_FAILED);
  });
});

describe("OAuthFlow provider wiring", () => {
  const providers: OAuthProvider[] = ["google", "apple", "facebook", "github"];
  for (const p of providers) {
    it(`${p} builds a login URL on its own provider path`, () => {
      const client = mockClient();
      const flow = createOAuthFlow(p, { client, state: mockState(), navigate: vi.fn() });
      expect(flow.loginUrl).toBe(`https://eq.test/api/auth/${p}/v1/login`);
    });
  }

  it("rejects an unknown provider at construction", () => {
    expect(() =>
      createOAuthFlow("twitter" as OAuthProvider, {
        client: mockClient(),
        state: mockState(),
        navigate: vi.fn(),
      }),
    ).toThrow();
  });
});

describe("OAuthFlow.finish", () => {
  it("exchange success -> transition(oauth) -> setUser, return target consumed, no leak", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(PROFILE, true));
    const state = mockState();
    const flow = createOAuthFlow("google", { client, state, navigate: vi.fn() });
    sessionStorage.setItem("eq:oauth-return", "/account");
    const res = await flow.finish();

    expect(res.ok).toBe(true);
    expect(res.returnTarget).toBe("/account");
    expect(state.transition).toHaveBeenCalledWith({ kind: "oauth" });
    const tOrder = stateOf(
      state.transition as unknown as { mock: { invocationCallOrder: number[] } },
    );
    const sOrder = stateOf(state.setUser as unknown as { mock: { invocationCallOrder: number[] } });
    expect(tOrder).toBeLessThan(sOrder);
    expect(state.setUser).toHaveBeenCalledWith(PROFILE);
    expect(consumeReturnTarget()).toBeNull();
  });

  it("opaque error code on probe-anonymous finish; no payload surfaced", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 400,
      data: null,
      error: { message: "sensitive internal detail" },
      rotated: false,
    });
    const state = mockState();
    state.probe.mockResolvedValueOnce({ kind: "anonymous" });
    const flow = createOAuthFlow("github", { client, state, navigate: vi.fn() });
    sessionStorage.setItem("eq:oauth-return", "/reader/1");
    const res = await flow.finish();

    expect(res.ok).toBe(false);
    expect(res.errorCode).toBe("oauth_failed");
    expect(res.returnTarget).toBe("/reader/1");
    expect(consumeReturnTarget()).toBeNull();
  });

  it("refreshCsrf runs when exchange did not rotate", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(PROFILE, false));
    const state = mockState();
    const flow = createOAuthFlow("google", { client, state, navigate: vi.fn() });
    await flow.finish();
    expect(client.refreshCsrf).toHaveBeenCalled();
  });
});

describe("OAuthFlow.reportFailure", () => {
  it("consumes the return target and surfaces opaque code only", () => {
    const client = mockClient();
    const flow = createOAuthFlow("apple", { client, state: mockState(), navigate: vi.fn() });
    sessionStorage.setItem("eq:oauth-return", "/go");
    const res = flow.reportFailure("access_denied");
    expect(res.ok).toBe(false);
    expect(res.errorCode).toBe("access_denied");
    expect(res.returnTarget).toBe("/go");
    expect(consumeReturnTarget()).toBeNull();
  });
});
