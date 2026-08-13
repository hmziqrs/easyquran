/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));
import type { AuthClient, AuthRequestResult, UserProfile } from "$lib/auth/auth-client";
import { createRegisterFlow } from "$lib/auth/flows.svelte";
import type { FlowStateLike } from "$lib/auth/flows.svelte";

const UNVERIFIED: UserProfile = {
  id: 9,
  name: "New",
  email: "new@eq.test",
  avatar_id: null,
  is_verified: false,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: null,
};

const VERIFIED: UserProfile = { ...UNVERIFIED, id: 10, is_verified: true };

function mockClient() {
  return {
    unsafeRequest: vi.fn(),
    refreshCsrf: vi.fn().mockResolvedValue(undefined),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
    refreshCsrf: ReturnType<typeof vi.fn>;
  };
}

function mockState(): FlowStateLike {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    reset: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "anonymous" }),
  } as unknown as FlowStateLike;
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}
function okStatus(status: number, data: unknown, rotated = false): AuthRequestResult<unknown> {
  return { ok: true, status, data, error: null, rotated };
}
function err(status: number, body: Record<string, unknown> = {}): AuthRequestResult<never> {
  return { ok: false, status, data: null, error: body as never, rotated: false };
}

describe("RegisterFlow register->login->verification", () => {
  it("register then login (rotated) -> unverified step; refreshCsrf NOT called since rotated handled by client", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(okStatus(201, UNVERIFIED)) // register
      .mockResolvedValueOnce(ok(UNVERIFIED, true)); // login rotated
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(true);
    expect(flow.step).toBe("unverified");
    expect(state.setUser).toHaveBeenCalledWith(UNVERIFIED);
    expect(state.transition).toHaveBeenCalledWith({ kind: "login" });
    expect(client.refreshCsrf).not.toHaveBeenCalled();
    expect(client.unsafeRequest).toHaveBeenNthCalledWith(
      1,
      "/auth/v1/register",
      expect.objectContaining({ method: "POST" }),
    );
    expect(client.unsafeRequest).toHaveBeenNthCalledWith(
      2,
      "/auth/v1/log_in",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("register then login rotated=false -> explicit refreshCsrf wait then proceed", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(okStatus(201, UNVERIFIED))
      .mockResolvedValueOnce(ok(VERIFIED, false));
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(true);
    expect(flow.step).toBe("done");
    expect(client.refreshCsrf).toHaveBeenCalledTimes(1);
    expect(state.setUser).toHaveBeenCalledWith(VERIFIED);
  });

  it("register -> login returns totp_required -> step totp, no setUser", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(okStatus(201, UNVERIFIED))
      .mockResolvedValueOnce(ok({ status: "totp_required", totp_token: "t-1" }));
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(false);
    expect(flow.step).toBe("totp");
    expect(state.setUser).not.toHaveBeenCalled();
  });

  it("register failure (400 field) -> form step, field error, login NOT attempted", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(
      err(400, { type: "invalid_input", context: { email: "already taken" } }),
    );
    const flow = createRegisterFlow({ client, state });
    flow.email = "taken@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(false);
    expect(flow.step).toBe("form");
    expect(flow.fieldErrors.email).toBe("already taken");
    expect(client.unsafeRequest).toHaveBeenCalledTimes(1);
  });

  it("register credential failure (401) -> uniform credential copy, no field leak", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(401, { type: "unauthorized" }));
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(false);
    expect(flow.step).toBe("form");
    expect(flow.genericError).toBe("Email or password is incorrect.");
    expect(flow.fieldErrors).toEqual({});
    expect(client.unsafeRequest).toHaveBeenCalledTimes(1);
  });

  it("register ok but login credential failure -> form step, uniform copy", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(okStatus(201, UNVERIFIED))
      .mockResolvedValueOnce(err(401, { type: "unauthorized" }));
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    const res = await flow.submit();
    expect(res).toBe(false);
    expect(flow.step).toBe("form");
    expect(flow.genericError).toBe("Email or password is incorrect.");
    expect(flow.fieldErrors).toEqual({});
    expect(state.setUser).not.toHaveBeenCalled();
  });

  it("does NOT call verification endpoints during registration", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(okStatus(201, UNVERIFIED))
      .mockResolvedValueOnce(ok(UNVERIFIED, true));
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "strong-password-1";
    await flow.submit();
    const calls = (client.unsafeRequest as ReturnType<typeof vi.fn>).mock.calls.map(
      (c) => c[0] as string,
    );
    expect(calls.some((p) => p.includes("email_verification"))).toBe(false);
    expect(calls).toEqual(["/auth/v1/register", "/auth/v1/log_in"]);
  });

  it("confirm_password mismatch -> aborts with field error, register NOT attempted", async () => {
    const client = mockClient();
    const state = mockState();
    const flow = createRegisterFlow({ client, state });
    flow.email = "new@eq.test";
    flow.password = "strong-password-1";
    flow.confirmPassword = "different-password";
    const res = await flow.submit();
    expect(res).toBe(false);
    expect(flow.fieldErrors.confirm_password).toBe("Password and confirm password do not match");
    expect(flow.step).toBe("form");
    expect(client.unsafeRequest).not.toHaveBeenCalled();
  });
});
