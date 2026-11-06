import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import type { UserProfile } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";

import { deviceIdFor, forgetDeviceIdFor, isMinorOwner, setDeviceIdFor } from "../identity";

/** A signed-in account. `is_minor` decides which identity path the owner takes. */
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

/** The stored id key for an owner, as `identity.ts` writes it. */
function storedKey(owner: string): string {
  return `easyquran.device.id.${owner}`;
}

describe("minor account device identity (A34)", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    authState.setUser(null);
  });

  it("never persists an id for a declared minor", () => {
    signIn(7, true);
    const first = deviceIdFor("7");
    expect(deviceIdFor("7")).toBe(first);
    expect(window.localStorage.getItem(storedKey("7"))).toBeNull();
  });

  it("gives a minor a random id, not the id stored for an adult", () => {
    setDeviceIdFor("7", "0190f2c4-7b1a-7c3d-9e4f-0123456789ab");
    signIn(7, true);
    expect(deviceIdFor("7")).not.toBe("0190f2c4-7b1a-7c3d-9e4f-0123456789ab");
  });

  it("keeps the persisted id for an adult account", () => {
    signIn(9, false);
    const stored = deviceIdFor("9");
    expect(window.localStorage.getItem(storedKey("9"))).toBe(stored);
  });

  it("only applies to the signed-in owner", () => {
    signIn(7, true);
    expect(isMinorOwner("7")).toBe(true);
    expect(isMinorOwner("8")).toBe(false);
  });

  it("forgets an id persisted before the flag was set", () => {
    signIn(9, false);
    deviceIdFor("9");
    expect(window.localStorage.getItem(storedKey("9"))).not.toBeNull();
    forgetDeviceIdFor("9");
    expect(window.localStorage.getItem(storedKey("9"))).toBeNull();
  });
});
