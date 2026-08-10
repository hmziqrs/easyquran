/* eslint-disable @typescript-eslint/unbound-method */
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import { createPasskeyFlow, type PasskeyFlowStateLike } from "$lib/auth/passkey-flow.svelte";
import type { AuthClient, AuthRequestResult, UserProfile } from "$lib/auth/auth-client";

const PROFILE: UserProfile = {
  id: 5,
  name: "Pk User",
  email: "pk@eq.test",
  avatar_id: null,
  is_verified: true,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: null,
};

function mockClient() {
  return {
    apiBase: "https://eq.test/api",
    unsafeRequest: vi.fn(),
    get: vi.fn(),
    ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    get: ReturnType<typeof vi.fn>;
    refreshCsrf: ReturnType<typeof vi.fn>;
  };
}

type PasskeyStateMock = PasskeyFlowStateLike & {
  transition: ReturnType<typeof vi.fn>;
  setUser: ReturnType<typeof vi.fn>;
  setTwoFaPending: ReturnType<typeof vi.fn>;
  probe: ReturnType<typeof vi.fn>;
};

function mockState(): PasskeyStateMock {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "authenticated", user: PROFILE }),
  } as unknown as PasskeyStateMock;
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}

function fakeBeginBody(challengeB64u = "AAAAAA"): unknown {
  return {
    publicKey: {
      challenge: challengeB64u,
      rpId: "easyquran.fyi",
      timeout: 60000,
      userVerification: "preferred",
      allowCredentials: [{ type: "public-key", id: "AAAA" }],
    },
  };
}

function fakeCredentialsGet(getImpl: () => Promise<Credential | null>): CredentialsContainer {
  return {
    get: vi.fn(getImpl) as unknown as CredentialsContainer["get"],
    create: vi.fn(),
    store: vi.fn(),
  } as unknown as CredentialsContainer;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("PasskeyFlow login cancellation is benign", () => {
  it("AbortError from navigator.credentials.get -> cancelled=true, NOT a server failure", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeBeginBody()));
    const creds = fakeCredentialsGet(() =>
      Promise.reject(new DOMException("User cancelled", "AbortError")),
    );
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();

    expect(res).toBe(false);
    expect(flow.cancelled).toBe(true);
    expect(flow.genericError).toBeNull();
    expect(flow.pending).toBe(false);
  });

  it("NotAllowedError is also treated as benign cancellation", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeBeginBody()));
    const creds = fakeCredentialsGet(() =>
      Promise.reject(new DOMException("disallowed", "NotAllowedError")),
    );
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(true);
    expect(flow.genericError).toBeNull();
  });

  it("null assertion (no selection) is cancellation, not server failure", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeBeginBody()));
    const creds = fakeCredentialsGet(() => Promise.resolve(null));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(true);
    expect(flow.genericError).toBeNull();
  });

  it("does not call /login/finish after cancellation", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeBeginBody()));
    const creds = fakeCredentialsGet(() =>
      Promise.reject(new DOMException("abort", "AbortError")),
    );
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    await flow.login();
    const paths = (client.unsafeRequest.mock.calls as [string, unknown][]).map(([p]) => p);
    expect(paths).toContain("/passkey/v1/login/begin");
    expect(paths).not.toContain("/passkey/v1/login/finish");
  });
});

describe("PasskeyFlow login success", () => {
  it("finish success -> refreshCsrf, transition(passkey), setUser", async () => {
    const client = mockClient();
    client.unsafeRequest
      .mockResolvedValueOnce(ok(fakeBeginBody()))
      .mockResolvedValueOnce(ok(PROFILE, true));
    const assertion = {
      id: "cred-1",
      rawId: new Uint8Array([1, 2, 3]).buffer,
      type: "public-key",
      response: {
        clientDataJSON: new Uint8Array([1]).buffer,
        authenticatorData: new Uint8Array([2]).buffer,
        signature: new Uint8Array([3]).buffer,
        userHandle: null,
      },
    } as unknown as PublicKeyCredential;
    const creds = fakeCredentialsGet(() => Promise.resolve(assertion));
    const state = mockState();
    const flow = createPasskeyFlow({ client, state, credentials: creds });
    const res = await flow.login();

    expect(res).toBe(true);
    expect(state.transition).toHaveBeenCalledWith({ kind: "passkey" });
    expect(state.setUser).toHaveBeenCalledWith(PROFILE);
    expect(state.setTwoFaPending).toHaveBeenCalledWith(false);
  });
});

describe("PasskeyFlow unsupported environment", () => {
  it("no credentials API -> generic error, no crash, not cancelled", async () => {
    const client = mockClient();
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: undefined });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(false);
    expect(flow.genericError).toBeTruthy();
  });
});
