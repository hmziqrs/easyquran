/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));
import { createTwoFactorFlow } from "$lib/auth/flows.svelte";
import type { AuthClient, AuthRequestResult, UserProfile } from "$lib/auth/auth-client";
import type { FlowStateLike } from "$lib/auth/flows.svelte";

const PROFILE: UserProfile = {
  id: 7,
  name: "Sara",
  email: "sara@eq.test",
  avatar_id: null,
  is_verified: true,
  role: "user",
  two_fa_enabled: false,
  oauth_provider: null,
};
const ENABLED: UserProfile = { ...PROFILE, two_fa_enabled: true };
const DISABLED: UserProfile = { ...PROFILE, two_fa_enabled: false };

function mockClient() {
  return {
    unsafeRequest: vi.fn(),
    refreshCsrf: vi.fn(),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as unknown as AuthClient & { unsafeRequest: ReturnType<typeof vi.fn> };
}

function mockState(): FlowStateLike & {
  transition: ReturnType<typeof vi.fn>;
  setUser: ReturnType<typeof vi.fn>;
} {
  return {
    transition: vi.fn().mockResolvedValue(undefined),
    setUser: vi.fn(),
    setTwoFaPending: vi.fn(),
    reset: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "authenticated" }),
  } as unknown as FlowStateLike & {
    transition: ReturnType<typeof vi.fn>;
    setUser: ReturnType<typeof vi.fn>;
  };
}

function ok<T>(data: T, rotated = false): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}
function err(
  status: number,
  body: Record<string, unknown> = {},
  rotated = false,
): AuthRequestResult<never> {
  return { ok: false, status, data: null, error: body as never, rotated };
}

function storageProbe(): { readKeys: string[] } {
  const readKeys: string[] = [];
  const proxy = (store: Storage | undefined, label: string) => {
    if (!store) return;
    const origSet = store.setItem.bind(store);
    store.setItem = (key: string, value: string) => {
      readKeys.push(`${label}.set:${key}`);
      return origSet(key, value);
    };
  };
  proxy(typeof localStorage !== "undefined" ? localStorage : undefined, "local");
  proxy(typeof sessionStorage !== "undefined" ? sessionStorage : undefined, "session");
  return { readKeys };
}

describe("TwoFactorFlow setup (secret stays in memory)", () => {
  it("setup success stores secret/otpauth/backup in memory, step verify, NO rotation (no transition)", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(
      ok(
        {
          secret: "BASE32SECRET",
          otpauth_url: "otpauth://totp/...",
          backup_codes: ["bc-1", "bc-2"],
        },
        false,
      ),
    );
    const probe = storageProbe();
    const flow = createTwoFactorFlow({ client, state });
    const res = await flow.setup();
    expect(res).toBe(true);
    expect(flow.step).toBe("verify");
    expect(flow.setupData?.secret).toBe("BASE32SECRET");
    expect(flow.setupData?.otpauthUrl).toBe("otpauth://totp/...");
    expect(flow.setupData?.backupCodes).toEqual(["bc-1", "bc-2"]);
    expect(state.transition).not.toHaveBeenCalled();
    expect(state.setUser).not.toHaveBeenCalled();
    expect(probe.readKeys).toEqual([]);
  });

  it("setup 403 verified-only -> next-step copy, no secret materialized", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(403, { type: "verification_required" }));
    const flow = createTwoFactorFlow({ client, state });
    const res = await flow.setup();
    expect(res).toBe(false);
    expect(flow.setupData).toBeNull();
    expect(flow.genericError).toMatch(/verify your email/i);
  });
});

describe("TwoFactorFlow verify (rotates only on success)", () => {
  it("verify success -> transition(two-fa-verify) before setUser, secret cleared, step enabled", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ secret: "S", otpauth_url: "u", backup_codes: [] }, false))
      .mockResolvedValueOnce(ok(ENABLED, true));
    const flow = createTwoFactorFlow({ client, state });
    await flow.setup();
    flow.verifyCode = "123456";
    const res = await flow.verify();
    expect(res).toBe(true);
    expect(state.transition).toHaveBeenCalledWith({ kind: "two-fa-verify" });
    const tOrder = (state.transition as unknown as { mock: { invocationCallOrder: number[] } }).mock
      .invocationCallOrder[0];
    const sOrder = (state.setUser as unknown as { mock: { invocationCallOrder: number[] } }).mock
      .invocationCallOrder[0];
    expect(tOrder).toBeLessThan(sOrder);
    expect(state.setUser).toHaveBeenCalledWith(ENABLED);
    expect(flow.setupData).toBeNull();
    expect(flow.step).toBe("enabled");
  });

  it("verify 400 -> code field error, still in verify step, no transition", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ secret: "S", otpauth_url: "u", backup_codes: [] }, false))
      .mockResolvedValueOnce(err(400, { type: "invalid_token" }));
    const flow = createTwoFactorFlow({ client, state });
    await flow.setup();
    flow.verifyCode = "000000";
    const res = await flow.verify();
    expect(res).toBe(false);
    expect(flow.fieldErrors.code).toBeDefined();
    expect(state.transition).not.toHaveBeenCalled();
    expect(state.setUser).not.toHaveBeenCalled();
  });

  it("verify 403 AUTH_008 (verified-only) -> next-step copy, not wrong-code field error", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest
      .mockResolvedValueOnce(ok({ secret: "S", otpauth_url: "u", backup_codes: [] }, false))
      .mockResolvedValueOnce(err(403, { type: "AUTH_008" }));
    const flow = createTwoFactorFlow({ client, state });
    await flow.setup();
    flow.verifyCode = "123456";
    const res = await flow.verify();
    expect(res).toBe(false);
    expect(flow.genericError).toMatch(/verify your email/i);
    expect(flow.fieldErrors.code).toBeUndefined();
    expect(state.transition).not.toHaveBeenCalled();
    expect(state.setUser).not.toHaveBeenCalled();
    expect(flow.step).not.toBe("enabled");
  });
});

describe("TwoFactorFlow disable (rotates)", () => {
  it("disable success -> body carries code, transition(two-fa-disable), setUser, step disabled", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(ok(DISABLED, true));
    const flow = createTwoFactorFlow({ client, state });
    flow.disableCode = "123456";
    const res = await flow.disable();
    expect(res).toBe(true);
    expect(state.transition).toHaveBeenCalledWith({ kind: "two-fa-disable" });
    expect(state.setUser).toHaveBeenCalledWith(DISABLED);
    expect(flow.step).toBe("disabled");
    expect(client.unsafeRequest).toHaveBeenCalledWith(
      "/auth/v1/2fa/disable",
      expect.objectContaining({
        method: "POST",
        body: expect.objectContaining({ code: expect.any(String) }),
      }),
    );
    expect(flow.disableCode).toBe("");
  });

  it("disable wrong code (401 invalid_token) -> code field error, no transition", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(401, { type: "invalid_token" }));
    const flow = createTwoFactorFlow({ client, state });
    flow.disableCode = "000000";
    const res = await flow.disable();
    expect(res).toBe(false);
    expect(flow.fieldErrors.code).toBeDefined();
    expect(flow.step).not.toBe("disabled");
    expect(state.transition).not.toHaveBeenCalled();
    expect(state.setUser).not.toHaveBeenCalled();
  });

  it("disable 403 verified-only -> next-step copy", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(err(403, { type: "verification_required" }));
    const flow = createTwoFactorFlow({ client, state });
    const res = await flow.disable();
    expect(res).toBe(false);
    expect(flow.genericError).toMatch(/verify your email/i);
  });
});

describe("TwoFactorFlow secret lifecycle", () => {
  it("clearSecrets wipes setup data", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(
      ok({ secret: "S", otpauth_url: "u", backup_codes: ["x"] }, false),
    );
    const flow = createTwoFactorFlow({ client, state });
    await flow.setup();
    expect(flow.setupData).not.toBeNull();
    flow.clearSecrets();
    expect(flow.setupData).toBeNull();
  });
});
