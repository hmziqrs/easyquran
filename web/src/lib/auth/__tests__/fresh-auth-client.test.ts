import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api" }));

import { createAccountClient } from "#lib/auth/account-client.js";
import { AuthClient, type AuthRequestResult, type WireObject } from "#lib/auth/auth-client.js";
import { FreshAuthClient, isFreshAuthenticationRequired } from "#lib/auth/fresh-auth-client.js";

function transport() {
  const client = new AuthClient();
  const get = vi.spyOn(client, "get");
  const unsafeRequest = vi.spyOn(client, "unsafeRequest");
  return { client, get, unsafeRequest };
}

function ok(data: WireObject): AuthRequestResult<WireObject> {
  return { ok: true, status: 200, data, error: null, rotated: false };
}

describe("Fresh authentication", () => {
  it("decodes status and fails closed on malformed success", async () => {
    const { client, get } = transport();
    get.mockResolvedValueOnce(ok({ ready: true, has_password: true, two_fa_enabled: true }));
    get.mockResolvedValueOnce(ok({ ready: "true", has_password: true, two_fa_enabled: false }));
    const fresh = new FreshAuthClient(client);
    expect(await fresh.status()).toEqual({ ready: true, has_password: true, two_fa_enabled: true });
    expect(await fresh.status()).toBeNull();
  });

  it("submits password and current factor through CSRF transport", async () => {
    const { client, unsafeRequest } = transport();
    unsafeRequest.mockResolvedValueOnce(ok({ ready: true }));
    expect(await new FreshAuthClient(client).confirm("current-secret", "123456")).toBeNull();
    expect(unsafeRequest).toHaveBeenCalledWith("/auth/v1/reauth", {
      method: "POST",
      body: { password: "current-secret", code: "123456" },
    });
  });

  it("recognizes release-build fresh-proof errors without private message", async () => {
    const { client, unsafeRequest } = transport();
    const error = { context: { reason: "fresh_auth_required" } };
    unsafeRequest.mockResolvedValue({ ok: false, status: 403, data: null, error, rotated: false });
    const result = await createAccountClient(client).unlinkIdentity("google");
    expect(isFreshAuthenticationRequired(error)).toBe(true);
    expect(result.status).toBe("error");
  });

  it("requires CSRF POST before navigating to linked provider authorization", async () => {
    const { client, unsafeRequest } = transport();
    unsafeRequest.mockResolvedValueOnce(
      ok({ url: "https://accounts.google.com/o/oauth2/v2/auth?state=s" }),
    );
    const result = await createAccountClient(client).startIdentityLink("google");
    expect(unsafeRequest).toHaveBeenCalledWith("/auth/google/v1/link", { method: "POST" });
    expect(result.data).toBe("https://accounts.google.com/o/oauth2/v2/auth?state=s");
  });

  it.each([
    "https://evil.test/oauth",
    "javascript:alert(1)",
    "https://github.com/oauth",
    "https://user@accounts.google.com/oauth",
  ])("refuses unexpected provider authorization URL %s", async (url) => {
    const { client, unsafeRequest } = transport();
    unsafeRequest.mockResolvedValueOnce(ok({ url }));
    expect((await createAccountClient(client).startIdentityLink("google")).data).toBeNull();
  });
});
