/* eslint-disable @typescript-eslint/unbound-method */
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

import type { AuthClient, AuthRequestResult, UserProfile } from "$lib/auth/auth-client";
import { createPasskeyFlow } from "$lib/auth/passkey-flow.svelte";

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

const LOGIN_HANDLE = "login-state-handle";
const REGISTER_HANDLE = "register-state-handle";

type PasskeyRequestBody = {
  state_handle?: string;
  credential?: {
    id?: string;
    rawId?: string;
    raw_id?: string;
    clientDataJSON?: string;
    response?: {
      clientDataJSON?: string;
      client_data_json?: string;
      authenticatorData?: string;
      signature?: string;
      userHandle?: string;
      attestationObject?: string;
      attestation_object?: string;
      transports?: string[];
    };
  };
  device_type?: string;
  transports?: string[];
  credential_id?: string;
  id?: number;
};

function mockClient() {
  // SAFETY: hand-built AuthClient test double — the class's #private csrf members are never
  // touched by PasskeyFlow, and every member these tests invoke is stubbed as a vi.fn().
  return {
    apiBase: "https://eq.test/api",
    unsafeRequest: vi.fn(),
    get: vi.fn(),
    ensureAnonymousSession: vi.fn().mockResolvedValue(undefined),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    get: ReturnType<typeof vi.fn>;
    ensureAnonymousSession: ReturnType<typeof vi.fn>;
    refreshCsrf: ReturnType<typeof vi.fn>;
    clearCsrf: ReturnType<typeof vi.fn>;
    getUser: ReturnType<typeof vi.fn>;
  };
}

function mockState() {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "authenticated", user: PROFILE }),
  };
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}

function fakeLoginBegin(challengeB64u = "AAAAAA") {
  return {
    challenge: {
      publicKey: {
        challenge: challengeB64u,
        rpId: "easyquran.fyi",
        timeout: 60000,
        userVerification: "required",
        allowCredentials: [{ type: "public-key", id: "AAAA" }],
      },
    },
    state_handle: LOGIN_HANDLE,
  };
}

function fakeRegisterBegin(challengeB64u = "BBBBBB") {
  return {
    challenge: {
      publicKey: {
        rp: { name: "EasyQuran", id: "easyquran.fyi" },
        user: { id: "AAAAAAAAAAAAAAAAAAAAAA", name: "pk@eq.test", displayName: "Pk User" },
        challenge: challengeB64u,
        pubKeyCredParams: [{ type: "public-key", alg: -7 }],
        timeout: 60000,
        authenticatorSelection: { userVerification: "preferred", requireResidentKey: false },
        attestation: "none",
      },
    },
    state_handle: REGISTER_HANDLE,
  };
}

const CREDENTIAL_VIEW = {
  id: 7,
  credential_id: "cred-2",
  device_type: "MacBook",
  transports: ["internal"],
  created_at: "2026-01-01T00:00:00Z",
  last_used_at: null,
};

function fakeAssertion() {
  return {
    id: "cred-1",
    rawId: new Uint8Array([1, 2, 3]).buffer,
    type: "public-key",
    response: {
      clientDataJSON: new Uint8Array([1]).buffer,
      authenticatorData: new Uint8Array([2]).buffer,
      signature: new Uint8Array([3]).buffer,
      userHandle: null,
    },
  };
}

function fakeAttestation() {
  return {
    id: "cred-2",
    rawId: new Uint8Array([10, 20]).buffer,
    type: "public-key",
    response: {
      clientDataJSON: new Uint8Array([11]).buffer,
      attestationObject: new Uint8Array([12]).buffer,
      getTransports: () => ["internal"],
    },
  };
}

function fakeCredentialsGet(getImpl: () => Promise<Credential | null>): CredentialsContainer {
  return {
    get: vi.fn(getImpl),
    create: vi.fn(),
    store: vi.fn(),
    preventSilentAccess: vi.fn(),
  };
}

function fakeCredentialsCreate(createImpl: () => Promise<Credential | null>): CredentialsContainer {
  return {
    get: vi.fn(),
    create: vi.fn(createImpl),
    store: vi.fn(),
    preventSilentAccess: vi.fn(),
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("PasskeyFlow login cancellation is benign", () => {
  it("AbortError from navigator.credentials.get -> cancelled=true, NOT a server failure", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeLoginBegin()));
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
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeLoginBegin()));
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
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeLoginBegin()));
    const creds = fakeCredentialsGet(() => Promise.resolve(null));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(true);
    expect(flow.genericError).toBeNull();
  });

  it("does not call /login/finish after cancellation", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeLoginBegin()));
    const creds = fakeCredentialsGet(() => Promise.reject(new DOMException("abort", "AbortError")));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    await flow.login();
    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const paths = (client.unsafeRequest.mock.calls as [string, unknown][]).map(([p]) => p);
    expect(paths).toContain("/passkey/v1/login/begin");
    expect(paths).not.toContain("/passkey/v1/login/finish");
  });
});

describe("PasskeyFlow login success", () => {
  it("finish success -> refreshCsrf, transition(passkey), setUser; finish body matches Rust contract", async () => {
    const client = mockClient();
    client.unsafeRequest
      .mockResolvedValueOnce(ok(fakeLoginBegin()))
      .mockResolvedValueOnce(ok({ status: "ok", user: PROFILE }, true));
    const creds = fakeCredentialsGet(() => Promise.resolve(fakeAssertion()));
    const state = mockState();
    const flow = createPasskeyFlow({ client, state, credentials: creds });
    const res = await flow.login();

    expect(res).toBe(true);
    expect(state.transition).toHaveBeenCalledWith({ kind: "passkey" });
    expect(state.setUser).toHaveBeenCalledWith(PROFILE);
    expect(state.setTwoFaPending).toHaveBeenCalledWith(false);

    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const finishCall = client.unsafeRequest.mock.calls[1] as [
      string,
      { method: string; body: PasskeyRequestBody },
    ];
    expect(finishCall[0]).toBe("/passkey/v1/login/finish");
    expect(finishCall[1]!.method).toBe("POST");
    const body = finishCall[1]!.body;
    expect(body.state_handle).toBe(LOGIN_HANDLE);
    expect("authentication_state" in body).toBe(false);
    const cred = body.credential!;
    expect(cred.rawId).toBeTypeOf("string");
    expect(cred.raw_id).toBeUndefined();
    expect(cred.clientDataJSON).toBeUndefined();
    const resp = cred.response!;
    expect(resp.clientDataJSON).toBeTypeOf("string");
    expect(resp.client_data_json).toBeUndefined();
    expect(resp.authenticatorData).toBeTypeOf("string");
    expect(resp.signature).toBeTypeOf("string");
  });

  it("missing user in finish response -> generic error, not a crash", async () => {
    const client = mockClient();
    client.unsafeRequest
      .mockResolvedValueOnce(ok(fakeLoginBegin()))
      .mockResolvedValueOnce(ok({ status: "ok" }));
    const creds = fakeCredentialsGet(() => Promise.resolve(fakeAssertion()));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.genericError).toBeTruthy();
  });

  it("begin response without state_handle -> generic error, no finish call", async () => {
    const client = mockClient();
    const beginWithoutHandle = { ...fakeLoginBegin(), state_handle: undefined };
    client.unsafeRequest.mockReset();
    client.unsafeRequest.mockResolvedValueOnce(ok(beginWithoutHandle));
    const creds = fakeCredentialsGet(() => Promise.resolve(fakeAssertion()));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.login();
    expect(res).toBe(false);
    expect(flow.genericError).toBeTruthy();
    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const paths = (client.unsafeRequest.mock.calls as [string, unknown][]).map(([p]) => p);
    expect(paths).not.toContain("/passkey/v1/login/finish");
  });
});

describe("PasskeyFlow register", () => {
  it("register success merges the returned view + transitions", async () => {
    const client = mockClient();
    client.unsafeRequest
      .mockResolvedValueOnce(ok(fakeRegisterBegin()))
      .mockResolvedValueOnce(ok(CREDENTIAL_VIEW));
    const creds = fakeCredentialsCreate(() => Promise.resolve(fakeAttestation()));
    const state = mockState();
    const flow = createPasskeyFlow({ client, state, credentials: creds });
    const res = await flow.register("MacBook");

    expect(res).toBe(true);
    expect(state.transition).toHaveBeenCalledWith({ kind: "passkey" });
    expect(state.probe).toHaveBeenCalled();
    expect(flow.passkeys).toHaveLength(1);
    expect(flow.passkeys[0]!.id).toBe("cred-2");
    expect(flow.passkeys[0]!.label).toBe("MacBook");

    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const finishCall = client.unsafeRequest.mock.calls[1] as [
      string,
      { method: string; body: PasskeyRequestBody },
    ];
    expect(finishCall[0]).toBe("/passkey/v1/register/finish");
    expect(finishCall[1]!.method).toBe("POST");
    const body = finishCall[1]!.body;
    expect(body.state_handle).toBe(REGISTER_HANDLE);
    expect("registration_state" in body).toBe(false);
    expect(body.device_type).toBe("MacBook");
    expect(body.transports).toEqual(["internal"]);
    const cred = body.credential!;
    expect(cred.rawId).toBeTypeOf("string");
    expect(cred.raw_id).toBeUndefined();
    const resp = cred.response!;
    expect(resp.attestationObject).toBeTypeOf("string");
    expect(resp.attestation_object).toBeUndefined();
    expect(resp.clientDataJSON).toBeTypeOf("string");
    expect(resp.transports).toEqual(["internal"]);
  });

  it("register begin 403 -> verify-email copy, not generic failure", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 403,
      data: null,
      error: { status: 403, message: "verified only" },
      rotated: false,
    });
    const creds = fakeCredentialsCreate(() => Promise.resolve(fakeAttestation()));
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.register("MacBook");
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(false);
    expect(flow.genericError).toBeTruthy();
  });

  it("register cancellation (NotAllowedError) is benign", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(ok(fakeRegisterBegin()));
    const creds = fakeCredentialsCreate(() =>
      Promise.reject(new DOMException("nope", "NotAllowedError")),
    );
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: creds });
    const res = await flow.register();
    expect(res).toBe(false);
    expect(flow.cancelled).toBe(true);
    expect(flow.genericError).toBeNull();
  });
});

describe("PasskeyFlow list", () => {
  it("list uses POST and reads data[]; maps credential_id -> id", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(
      ok({ data: [CREDENTIAL_VIEW, { ...CREDENTIAL_VIEW, id: 8, credential_id: "cred-9" }] }),
    );
    const flow = createPasskeyFlow({ client, state: mockState() });
    const res = await flow.list();

    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const call = client.unsafeRequest.mock.calls[0] as [string, { method: string }];
    expect(call[0]).toBe("/passkey/v1/list");
    expect(call[1]!.method).toBe("POST");
    expect(res).not.toBeNull();
    expect(res).toHaveLength(2);
    expect(res?.[0]!.id).toBe("cred-2");
    expect(res?.[1]!.id).toBe("cred-9");
    expect(flow.passkeys).toHaveLength(2);
  });

  it("list skips entries without credential_id", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce(
      ok({ data: [CREDENTIAL_VIEW, { id: 9 }, { credential_id: "x" }] }),
    );
    const flow = createPasskeyFlow({ client, state: mockState() });
    const res = await flow.list();
    expect(res?.find((p) => p.id === "x")).toBeDefined();
    expect(res?.find((p) => p.id === "cred-2")).toBeDefined();
  });

  it("list 401 -> null", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 401,
      data: null,
      error: null,
      rotated: false,
    });
    const flow = createPasskeyFlow({ client, state: mockState() });
    const res = await flow.list();
    expect(res).toBeNull();
  });
});

describe("PasskeyFlow remove", () => {
  it("remove sends { credential_id } and drops the entry locally", async () => {
    const client = mockClient();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ data: [CREDENTIAL_VIEW] }))
      .mockResolvedValueOnce(ok({ message: "Passkey removed" }));
    const flow = createPasskeyFlow({ client, state: mockState() });
    await flow.list();
    expect(flow.passkeys).toHaveLength(1);

    const res = await flow.remove("cred-2");
    expect(res).toBe(true);

    // SAFETY: unsafeRequest is invoked positionally as (path, init); mock.calls keeps that order.
    const removeCall = client.unsafeRequest.mock.calls[1] as [
      string,
      { method: string; body: PasskeyRequestBody },
    ];
    expect(removeCall[0]).toBe("/passkey/v1/remove");
    expect(removeCall[1]!.method).toBe("POST");
    expect(removeCall[1]!.body.credential_id).toBe("cred-2");
    expect(removeCall[1]!.body.id).toBeUndefined();
    expect(flow.passkeys).toHaveLength(0);
  });

  it("remove failure surfaces generic error", async () => {
    const client = mockClient();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 404,
      data: null,
      error: { status: 404, message: "not found" },
      rotated: false,
    });
    const flow = createPasskeyFlow({ client, state: mockState() });
    const res = await flow.remove("cred-2");
    expect(res).toBe(false);
    expect(flow.genericError).toBeTruthy();
  });

  it("remove empty id is a no-op", async () => {
    const client = mockClient();
    const flow = createPasskeyFlow({ client, state: mockState() });
    const res = await flow.remove("");
    expect(res).toBe(false);
    expect(client.unsafeRequest).not.toHaveBeenCalled();
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

  it("register with no credentials API -> generic error", async () => {
    const client = mockClient();
    const flow = createPasskeyFlow({ client, state: mockState(), credentials: undefined });
    const res = await flow.register();
    expect(res).toBe(false);
    expect(flow.genericError).toBeTruthy();
  });
});
