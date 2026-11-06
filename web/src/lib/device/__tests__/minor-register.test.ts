import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import type { UserProfile } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";

import { ensureDeviceRegistered } from "../register";

/** Every request the register path would make. A minor account must make none. */
const fetchMock = vi.fn<(input: string, init?: RequestInit) => Promise<Response>>(
  async () => new Response("{}", { status: 200 }),
);

function signIn(id: number, isMinor: boolean): void {
  const user: UserProfile = {
    id,
    name: "fixture",
    email: `user${id}@example.com`,
    avatar_id: null,
    is_verified: true,
    is_minor: isMinor,
    role: "user",
    two_fa_enabled: false,
    oauth_provider: null,
  };
  authState.setUser(user);
}

describe("device registration for a minor account (A34)", () => {
  beforeEach(() => {
    window.localStorage.clear();
    fetchMock.mockClear();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    authState.setUser(null);
    vi.unstubAllGlobals();
  });

  it("makes no challenge and no register call, and is not a failure", async () => {
    signIn(7, true);
    const ok = await ensureDeviceRegistered(7, "push-token-fixture");
    expect(ok).toBe(true);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("still registers an adult account", async () => {
    signIn(8, false);
    await ensureDeviceRegistered(8);
    const urls = fetchMock.mock.calls.map((call) => String(call[0]));
    expect(urls.some((url) => url.endsWith("/device/v1/register"))).toBe(true);
  });
});
