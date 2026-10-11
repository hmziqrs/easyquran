import { describe, expect, it } from "vitest";

import { pendingTotpToken } from "../pending-proof";

describe("OAuth factor continuation", () => {
  it("accepts only the pending factor envelope", () => {
    const token = "a".repeat(64);
    expect(pendingTotpToken({ status: "totp_required", totp_token: token })).toBe(token);
    expect(pendingTotpToken({ status: "none", totp_token: token })).toBeNull();
    expect(pendingTotpToken({ id: 7, totp_token: token })).toBeNull();
  });

  it.each([
    null,
    {},
    { status: "totp_required" },
    { status: "totp_required", totp_token: 7 },
    { status: "totp_required", totp_token: "expired" },
  ])("rejects malformed proof: %j", (data) => {
    expect(pendingTotpToken(data)).toBeNull();
  });
});
