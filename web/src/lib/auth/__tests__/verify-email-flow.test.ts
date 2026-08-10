/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));
import { createVerifyEmailFlow } from "$lib/auth/flows.svelte";
import type { AuthClient, AuthRequestResult } from "$lib/auth/auth-client";
import type { FlowStateLike } from "$lib/auth/flows.svelte";

function mockClient() {
  return {
    unsafeRequest: vi.fn(),
    refreshCsrf: vi.fn(),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & { unsafeRequest: ReturnType<typeof vi.fn> };
}

function mockState(): FlowStateLike & { probe: ReturnType<typeof vi.fn> } {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    reset: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "authenticated" }),
  } as unknown as FlowStateLike & { probe: ReturnType<typeof vi.fn> };
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}
function err(status: number, body: Record<string, unknown> = {}): AuthRequestResult<never> {
  return { ok: false, status, data: null, error: body as never, rotated: false };
}

describe("VerifyEmailFlow confirm", () => {
  it("verify success -> re-probes user, verified flag set", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(
      ok({ message: "Email verified successfully" }, true),
    );
    const flow = createVerifyEmailFlow({ client, state });
    flow.code = "123456";
    const res = await flow.verify();
    expect(res).toBe(true);
    expect(flow.verified).toBe(true);
    expect(state.probe).toHaveBeenCalledTimes(1);
    expect(client.unsafeRequest).toHaveBeenCalledWith(
      "/email_verification/v1/verify",
      expect.objectContaining({ method: "POST", body: { code: "123456" } }),
    );
  });

  it("verified-only 403 -> alreadyVerified true, explains next step instead of looping", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(403, { type: "verification_required" }));
    const flow = createVerifyEmailFlow({ client, state });
    const res = await flow.verify();
    expect(res).toBe(false);
    expect(flow.alreadyVerified).toBe(true);
    expect(flow.verified).toBe(false);
    expect(flow.genericError).toBeNull();
  });

  it("400 invalid code -> code field error", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(
      err(400, { type: "invalid_input", context: { code: "wrong code" } }),
    );
    const flow = createVerifyEmailFlow({ client, state });
    const res = await flow.verify();
    expect(res).toBe(false);
    expect(flow.fieldErrors.code).toBe("wrong code");
  });

  it("429 resend -> rate-limit message, no crash", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(429, { retry_after: 60 }));
    const flow = createVerifyEmailFlow({ client, state });
    const res = await flow.resend();
    expect(res).toBe(false);
    expect(flow.genericError).toMatch(/60s/);
  });

  it("resend success -> uniform account-exists copy", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "sent" }));
    const flow = createVerifyEmailFlow({ client, state });
    const res = await flow.resend();
    expect(res).toBe(true);
    expect(flow.lastResentAt).not.toBeNull();
    expect(flow.genericError).toMatch(/account exists/i);
  });

  it("transport failure on verify -> network message, verified stays false", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockRejectedValueOnce(new Error("net"));
    const flow = createVerifyEmailFlow({ client, state });
    const res = await flow.verify();
    expect(res).toBe(false);
    expect(flow.verified).toBe(false);
    expect(flow.genericError).toMatch(/network/i);
  });
});
