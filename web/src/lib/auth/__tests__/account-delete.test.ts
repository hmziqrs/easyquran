import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  PUBLIC_TURNSTILE_SITE_KEY: undefined,
}));

import { createAccountClient } from "#lib/auth/account-client.js";
import type { AuthClient, AuthRequestResult } from "#lib/auth/auth-client.js";

function catalog(locale: "en" | "ar"): Record<string, string> {
  // Vitest runs from the web/ root (same as catalogue-sha-guard.test.ts).
  return JSON.parse(readFileSync(resolve(process.cwd(), "messages", `${locale}.json`), "utf8"));
}

function mockClient(unsafeRequest: ReturnType<typeof vi.fn>): AuthClient {
  // SAFETY: test double; AccountClient only calls unsafeRequest on this path.
  return { unsafeRequest } as never;
}

describe("self-service deletion legal copy", () => {
  it.each(["en", "ar"] as const)(
    "%s privacy text no longer asks the user to request deletion",
    (locale) => {
      const copy = catalog(locale);
      expect(copy.legal_privacy_accounts_body).not.toMatch(/contact us|تواصل معنا/);
      expect(copy.legal_privacy_choices_body).not.toMatch(/ask us to delete|طلب حذفه/);
    },
  );
});

describe("AccountClient.deleteAccount", () => {
  it("posts the typed email to the self-delete route and reports ok", async () => {
    const unsafeRequest = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      data: { receipt_id: 1 },
      error: null,
      rotated: false,
    } satisfies AuthRequestResult<unknown>);

    const res = await createAccountClient(mockClient(unsafeRequest)).deleteAccount("a@example.com");

    expect(unsafeRequest).toHaveBeenCalledWith("/user/v1/delete", {
      method: "POST",
      body: { confirmEmail: "a@example.com" },
    });
    expect(res.status).toBe("ok");
  });

  it("reports error, not anonymous, when fresh auth is missing", async () => {
    const unsafeRequest = vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      data: null,
      error: { context: { reason: "fresh_auth_required" } },
      rotated: false,
    });

    const res = await createAccountClient(mockClient(unsafeRequest)).deleteAccount("a@example.com");

    expect(res.status).toBe("error");
  });
});
