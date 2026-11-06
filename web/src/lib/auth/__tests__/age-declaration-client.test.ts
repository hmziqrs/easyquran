import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({ PUBLIC_API_BASE_URL: "/api", PUBLIC_FCM_VAPID_KEY: "" }));

import { AccountClient } from "#lib/auth/account-client.js";
import type { AuthRequestResult, WireObject } from "#lib/auth/auth-client.js";

/** A test double: `declareAge` only calls `unsafeRequest`. */
function mockClient(reply: AuthRequestResult<WireObject>) {
  const unsafeRequest = vi.fn().mockResolvedValue(reply);
  // SAFETY: test double; AccountClient only calls unsafeRequest on this path.
  const client = new AccountClient({ unsafeRequest } as never);
  return { client, unsafeRequest };
}

describe("declareAge (A34)", () => {
  it("posts the date once to the age endpoint and reads back the flag, not the date", async () => {
    const { client, unsafeRequest } = mockClient({
      ok: true,
      status: 200,
      data: {
        id: 3,
        email: "a@b.c",
        is_minor: false,
        age_declared_at: "2026-11-06T00:00:00+00:00",
      },
      error: null,
      rotated: false,
    });
    const res = await client.declareAge("1990-06-15");
    expect(unsafeRequest).toHaveBeenCalledWith("/user/v1/age/declare", {
      method: "POST",
      body: { date_of_birth: "1990-06-15" },
    });
    expect(res.status).toBe("ok");
    expect(res.data?.age_declared).toBe(true);
    expect(res.data?.is_minor).toBe(false);
  });

  it("reports a 409 with its http status so the page can say it is already recorded", async () => {
    const { client } = mockClient({
      ok: false,
      status: 409,
      data: null,
      error: null,
      rotated: false,
    });
    const res = await client.declareAge("2015-01-01");
    expect(res.status).toBe("error");
    expect(res.httpStatus).toBe(409);
    expect(res.data).toBeNull();
  });
});
