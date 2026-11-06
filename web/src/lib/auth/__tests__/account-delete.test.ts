import { readFileSync } from "node:fs";

import { describe, expect, it, vi } from "vite-plus/test";

import { createAccountClient } from "#lib/auth/account-client.js";
import type { AuthClient, AuthRequestResult } from "#lib/auth/auth-client.js";

function catalog(locale: "en" | "ar"): Record<string, string> {
  const url = new URL(`../../../../messages/${locale}.json`, import.meta.url);
  return JSON.parse(readFileSync(url, "utf8"));
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
