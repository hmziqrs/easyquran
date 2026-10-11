import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import { authClient } from "#lib/auth/auth-client.js";
import { registerTokenWithServer, unregisterTokenFromServer } from "#lib/firebase/messaging.js";
import { asString } from "#lib/storage/decoders.js";

const requests: { url: string; init: RequestInit }[] = [];

beforeEach(() => {
  authClient.clearCsrf();
  requests.length = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn((url: string, init: RequestInit = {}) => {
      requests.push({ url, init });
      if (url.endsWith("/csrf/v1/generate"))
        return Promise.resolve(Response.json({ token: "csrf-proof" }));
      return Promise.resolve(Response.json({ id: 1 }));
    }),
  );
});

afterEach(() => vi.unstubAllGlobals());

describe("push registration transport", () => {
  it("sends CSRF and captured owner on registration and deletion", async () => {
    expect(await registerTokenWithServer("private-token", 7)).toBe(true);
    expect(await unregisterTokenFromServer("private-token", 7)).toBe(true);
    expect(requests.map((request) => request.url)).toEqual([
      "/api/csrf/v1/generate",
      "/api/device/v1/register",
      "/api/device/v1/delete",
    ]);
    for (const request of requests.slice(1)) {
      expect(request.init.headers).toMatchObject({ "csrf-token": "csrf-proof" });
      expect(request.init.credentials).toBe("include");
      expect(JSON.parse(asString(request.init.body) ?? "null")).toMatchObject({
        token: "private-token",
        expected_user_id: 7,
      });
    }
  });

  it("does not treat a changed-session owner rejection as successful registration", async () => {
    await authClient.ensureAnonymousSession();
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          Response.json({ message: "Push subscription account changed" }, { status: 403 }),
        ),
      ),
    );
    expect(await registerTokenWithServer("private-token", 7)).toBe(false);
  });
});
