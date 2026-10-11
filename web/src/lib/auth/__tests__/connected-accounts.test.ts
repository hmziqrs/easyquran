import { afterEach, describe, expect, it, vi } from "vite-plus/test";
vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import {
  canUnlinkIdentity,
  countSignInMethods,
  identityProviderLabel,
  type IdentityProvider,
  type IdentitySummary,
  type LinkedIdentity,
} from "#lib/auth/account-client.js";
import { consumeLinkProvider, rememberLinkProvider } from "#lib/auth/link-attempt.js";

function identity(provider: IdentityProvider): LinkedIdentity {
  return { provider, providerUserId: `${provider}-1`, createdAt: "2026-01-01T00:00:00Z" };
}

function summary(over: Partial<IdentitySummary>): IdentitySummary {
  return {
    identities: [],
    hasPassword: false,
    passkeyCount: 0,
    canUnlinkAny: true,
    linkableProviders: [],
    ...over,
  };
}

afterEach(() => {
  sessionStorage.clear();
});

describe("countSignInMethods / canUnlinkIdentity", () => {
  it("counts identities, the password and the passkey factor separately", () => {
    expect(countSignInMethods(summary({}))).toBe(0);
    expect(countSignInMethods(summary({ hasPassword: true }))).toBe(1);
    expect(countSignInMethods(summary({ passkeyCount: 3 }))).toBe(1);
    expect(countSignInMethods(summary({ identities: [identity("github")], passkeyCount: 2 }))).toBe(
      2,
    );
    expect(
      countSignInMethods(
        summary({ identities: [identity("github"), identity("google")], hasPassword: true }),
      ),
    ).toBe(3);
  });

  it("blocks unlink while the identity is the only method", () => {
    expect(canUnlinkIdentity(summary({ identities: [identity("github")] }))).toBe(false);
  });

  it("allows unlink when a password or another identity exists", () => {
    expect(
      canUnlinkIdentity(summary({ identities: [identity("github")], hasPassword: true })),
    ).toBe(true);
    expect(
      canUnlinkIdentity(summary({ identities: [identity("github"), identity("google")] })),
    ).toBe(true);
  });

  it("allows unlink when a passkey backs the account", () => {
    expect(canUnlinkIdentity(summary({ identities: [identity("github")], passkeyCount: 1 }))).toBe(
      true,
    );
  });

  it("reports no unlinkable identity when only a password exists", () => {
    expect(canUnlinkIdentity(summary({ hasPassword: true }))).toBe(false);
  });
});

describe("identityProviderLabel", () => {
  it("uses the provider brand name", () => {
    expect(identityProviderLabel("github")).toBe("GitHub");
    expect(identityProviderLabel("google")).toBe("Google");
    expect(identityProviderLabel("discord")).toBe("Discord");
  });
});

describe("link-attempt stash", () => {
  it("remembers the attempted provider and consumes it once", () => {
    rememberLinkProvider("github");
    expect(consumeLinkProvider()).toBe("github");
    expect(consumeLinkProvider()).toBeNull();
  });

  it("ignores unknown stored values", () => {
    sessionStorage.setItem("eq:link-provider", "steam");
    expect(consumeLinkProvider()).toBeNull();
  });
});
