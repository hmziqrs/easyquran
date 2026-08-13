/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));
import type { AuthClient, AuthRequestResult } from "$lib/auth/auth-client";
import { createForgotPasswordFlow } from "$lib/auth/flows.svelte";
import type { FlowStateLike } from "$lib/auth/flows.svelte";

function mockClient() {
  return {
    unsafeRequest: vi.fn(),
    refreshCsrf: vi.fn(),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & { unsafeRequest: ReturnType<typeof vi.fn> };
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
function err(status: number, body: Record<string, unknown> = {}): AuthRequestResult<never> {
  return { ok: false, status, data: null, error: body as never, rotated: false };
}

function storageProbe(): { readKeys: string[] } {
  const readKeys: string[] = [];
  const proxy = (store: Storage | undefined, label: string) => {
    if (!store) return;
    const origSet = store.setItem.bind(store);
    store.setItem = (key: string, value: string) => {
      readKeys.push(`${label}.set:${key}=${value}`);
      return origSet(key, value);
    };
  };
  proxy(typeof localStorage !== "undefined" ? localStorage : undefined, "local");
  proxy(typeof sessionStorage !== "undefined" ? sessionStorage : undefined, "session");
  return { readKeys };
}

describe("ForgotPasswordFlow request (uniform account-existence copy)", () => {
  it("200 -> successMessage copy (green), genericError clear, step verify", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "If an account exists..." }));
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "maybe@eq.test";
    const res = await flow.request();
    expect(res).toBe(true);
    expect(flow.step).toBe("verify");
    expect(flow.successMessage).toMatch(/account exists/i);
    expect(flow.genericError).toBeNull();
  });

  it("masquerade failure (404) -> account-existence copy surfaces as successMessage (green), not serverError", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(404));
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "no-such-account@eq.test";
    const res = await flow.request();
    expect(res).toBe(false);
    expect(flow.step).toBe("request");
    expect(flow.successMessage).toMatch(/account exists/i);
    expect(flow.genericError).toBeNull();
  });

  it("429 rate-limit on request -> rate-limit message (does not masquerade as account-exists)", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(429, { retry_after: 60 }));
    const flow = createForgotPasswordFlow({ client, state });
    const res = await flow.request();
    expect(res).toBe(false);
    expect(flow.step).toBe("request");
    expect(flow.genericError).toMatch(/60s/);
    expect(flow.successMessage).toBeNull();
  });
});

describe("ForgotPasswordFlow verify -> reset token in MEMORY only", () => {
  it("verify success stores reset_token in memory, advances to reset step", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "sent" }));
    client.unsafeRequest.mockResolvedValueOnce(ok({ reset_token: "RESET-TOKEN-SECRET" }));
    const probe = storageProbe();
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "x@eq.test";
    await flow.request();
    const res = await flow.verifyCode();
    expect(res).toBe(true);
    expect(flow.step).toBe("reset");
    expect(flow.resetTokenInMemory).toBe(true);
    expect(probe.readKeys).toEqual([]);
  });

  it("reset token never appears in URL/path or storage: reset() posts it in body only", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ message: "sent" }))
      .mockResolvedValueOnce(ok({ reset_token: "SECRET-TOK" }))
      .mockResolvedValueOnce(ok({ message: "reset done" }));
    const probe = storageProbe();
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "x@eq.test";
    flow.code = "12345678";
    flow.password = "new-strong-password";
    flow.confirmPassword = "new-strong-password";
    await flow.request();
    await flow.verifyCode();
    const res = await flow.reset();
    expect(res).toBe(true);
    expect(probe.readKeys).toEqual([]);
    const resetCall = (client.unsafeRequest as ReturnType<typeof vi.fn>).mock.calls.find(
      (c) => (c[0] as string) === "/forgot_password/v1/reset",
    );
    expect(resetCall).toBeDefined();
    const path = resetCall![0] as string;
    expect(path).not.toContain("SECRET-TOK");
    const body = (resetCall![1] as { body: Record<string, string> }).body;
    expect(body.reset_token).toBe("SECRET-TOK");
    expect(flow.resetTokenInMemory).toBe(false);
  });

  it("reset without token -> expired guard, no network call", async () => {
    const client = mockClient();
    const state = mockState();
    const flow = createForgotPasswordFlow({ client, state });
    const res = await flow.reset();
    expect(res).toBe(false);
    expect(flow.step).toBe("request");
    expect(flow.genericError).toMatch(/expired/i);
    expect(client.unsafeRequest).not.toHaveBeenCalled();
  });

  it("reset password mismatch -> field error on confirm_password", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ message: "sent" }))
      .mockResolvedValueOnce(ok({ reset_token: "TOK-2" }))
      .mockResolvedValueOnce(
        err(400, {
          type: "invalid_input",
          context: { confirm_password: "Password and confirm password do not match" },
        }),
      );
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "x@eq.test";
    await flow.request();
    await flow.verifyCode();
    const res = await flow.reset();
    expect(res).toBe(false);
    expect(flow.fieldErrors.confirm_password).toContain("match");
  });

  it("clearSecrets wipes the in-memory reset token", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ message: "sent" }))
      .mockResolvedValueOnce(ok({ reset_token: "TOK-3" }));
    const flow = createForgotPasswordFlow({ client, state });
    flow.email = "x@eq.test";
    await flow.request();
    await flow.verifyCode();
    expect(flow.resetTokenInMemory).toBe(true);
    flow.clearSecrets();
    expect(flow.resetTokenInMemory).toBe(false);
  });
});
