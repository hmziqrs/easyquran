/* eslint-disable @typescript-eslint/unbound-method */
import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));
import type { AuthClient, AuthRequestResult } from "$lib/auth/auth-client";
import { createLogoutFlow } from "$lib/auth/flows.svelte";

function mockClient() {
  // SAFETY: hand-built AuthClient test double — the class's #private csrf members are never
  // touched by LogoutFlow, and every member these tests invoke is stubbed as a vi.fn().
  return {
    unsafeRequest: vi.fn(),
    refreshCsrf: vi.fn(),
    clearCsrf: vi.fn(),
    getUser: vi.fn(),
  } as AuthClient & {
    unsafeRequest: ReturnType<typeof vi.fn>;
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
    reset: vi.fn(),
    probe: vi.fn().mockResolvedValue({ kind: "anonymous" }),
  };
}

function ok<T>(data: T, rotated = true): AuthRequestResult<T> {
  return { ok: true, status: 200, data, error: null, rotated };
}

describe("LogoutFlow clearing + purge + anonymous probe", () => {
  it("posts log_out, clears CSRF, transitions(logout), resets, probes anonymous", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce(ok({ message: "Logged out" }, true));
    const flow = createLogoutFlow({ client, state });
    const res = await flow.run();
    expect(res).toBe(true);
    expect(client.unsafeRequest).toHaveBeenCalledWith(
      "/auth/v1/log_out",
      expect.objectContaining({ method: "POST" }),
    );
    expect(client.clearCsrf).toHaveBeenCalledTimes(1);
    expect(state.transition).toHaveBeenCalledWith({ kind: "logout" });
    expect(state.reset).toHaveBeenCalledTimes(1);
    expect(state.probe).toHaveBeenCalledTimes(1);
    expect(flow.anonymous).toBe(true);

    const clearOrder = client.clearCsrf.mock.invocationCallOrder[0]!;
    const transitionOrder = state.transition.mock.invocationCallOrder[0]!;
    const resetOrder = state.reset.mock.invocationCallOrder[0]!;
    const probeOrder = state.probe.mock.invocationCallOrder[0]!;
    expect(clearOrder).toBeLessThan(transitionOrder);
    expect(transitionOrder).toBeLessThan(resetOrder);
    expect(resetOrder).toBeLessThan(probeOrder);
  });

  it("still completes the local teardown when backend reports already-anonymous (401)", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 401,
      data: null,
      error: { type: "unauthorized" },
      rotated: false,
    });
    const flow = createLogoutFlow({ client, state });
    const res = await flow.run();
    expect(res).toBe(true);
    expect(client.clearCsrf).toHaveBeenCalled();
    expect(state.transition).toHaveBeenCalledWith({ kind: "logout" });
    expect(state.reset).toHaveBeenCalled();
    expect(state.probe).toHaveBeenCalled();
    expect(flow.anonymous).toBe(true);
  });

  it("server 5xx is reported as failure (does not silently wipe local state)", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockResolvedValueOnce({
      ok: false,
      status: 503,
      data: null,
      error: { type: "internal_server_error" },
      rotated: false,
    });
    const flow = createLogoutFlow({ client, state });
    const res = await flow.run();
    expect(res).toBe(false);
    expect(flow.genericError).toBeDefined();
    expect(client.clearCsrf).not.toHaveBeenCalled();
    expect(state.reset).not.toHaveBeenCalled();
  });

  it("transport failure -> network message, no partial teardown", async () => {
    const client = mockClient();
    const state = mockState();
    client.unsafeRequest.mockRejectedValueOnce(new Error("net"));
    const flow = createLogoutFlow({ client, state });
    const res = await flow.run();
    expect(res).toBe(false);
    expect(flow.genericError).toMatch(/network/i);
    expect(client.clearCsrf).not.toHaveBeenCalled();
    expect(state.reset).not.toHaveBeenCalled();
  });
});
