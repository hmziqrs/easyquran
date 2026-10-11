import { describe, expect, it } from "vite-plus/test";

import {
  failureMessage,
  isOpaqueFailureCode,
  OAUTH_FAILURE_HEADING,
  OAUTH_FAILURE_MESSAGE,
} from "../oauth-failure";

describe("oauth-failure failureMessage is uniform + leak-free", () => {
  const LEAKY = [
    "oauth_failed",
    "oauth_denied",
    "oauth_state_mismatch",
    "",
    "  ",
    "<script>alert(1)</script>",
    "provider=google invalid_grant: bad code",
    "state=abc123&code=xyz789",
    "https://evil.example/steal",
    "//evil.example",
    "email=user@eq.test",
    "Bearer eyJhbGciOi.test.token",
    "x".repeat(5000),
    "DROP TABLE users;--",
    "javascript:alert(document.cookie)",
  ];

  for (const ec of LEAKY) {
    it(`returns the uniform constant for ec=${JSON.stringify(ec).slice(0, 40)}`, () => {
      const msg = failureMessage(ec);
      expect(msg).toBe(OAUTH_FAILURE_MESSAGE);
    });
  }

  it("never reflects the ec value in the message (no payload leak)", () => {
    for (const ec of LEAKY) {
      if (!ec.trim()) continue;
      const msg = failureMessage(ec);
      expect(msg).not.toContain(ec);
      expect(msg.toLowerCase()).not.toContain("code=");
      expect(msg.toLowerCase()).not.toContain("state=");
      expect(msg).not.toContain("<");
      expect(msg).not.toContain("http");
      expect(msg).not.toContain("@");
    }
  });

  it("message + heading contain no provider-identifying text", () => {
    expect(OAUTH_FAILURE_MESSAGE).not.toMatch(/google|apple|facebook|github/i);
    expect(OAUTH_FAILURE_HEADING).not.toMatch(/google|apple|facebook|github/i);
  });

  it("treats null/undefined the same as a real code", () => {
    expect(failureMessage(null)).toBe(OAUTH_FAILURE_MESSAGE);
    expect(failureMessage(undefined)).toBe(OAUTH_FAILURE_MESSAGE);
  });
});

describe("oauth-failure isOpaqueFailureCode allowlist", () => {
  it("accepts lowercase snake_case opaque codes", () => {
    expect(isOpaqueFailureCode("oauth_failed")).toBe(true);
    expect(isOpaqueFailureCode("oauth_denied")).toBe(true);
    expect(isOpaqueFailureCode("a")).toBe(true);
    expect(isOpaqueFailureCode("abc_def_123")).toBe(true);
  });

  it("rejects anything that could carry a provider payload or injection", () => {
    expect(isOpaqueFailureCode("")).toBe(false);
    expect(isOpaqueFailureCode("OAuth Failed")).toBe(false);
    expect(isOpaqueFailureCode("<script>")).toBe(false);
    expect(isOpaqueFailureCode("code=xyz")).toBe(false);
    expect(isOpaqueFailureCode("a b c")).toBe(false);
    expect(isOpaqueFailureCode("x".repeat(49))).toBe(false);
    expect(isOpaqueFailureCode(null)).toBe(false);
    expect(isOpaqueFailureCode(123)).toBe(false);
    expect(isOpaqueFailureCode(undefined)).toBe(false);
  });
});
