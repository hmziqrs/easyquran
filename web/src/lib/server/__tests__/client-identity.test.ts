import { describe, expect, it } from "vite-plus/test";

import { trustedClientIp } from "#lib/server/client-identity.js";

const MARKER = "a".repeat(64);

function request(headers: Record<string, string>): Request {
  return new Request("https://example.test/t/en/sahih/page/1", { headers });
}

describe("trustedClientIp", () => {
  it("uses the socket peer when no marker is configured", () => {
    const req = request({ "cf-connecting-ip": "198.51.100.7" });
    expect(trustedClientIp(req, "10.0.0.2", undefined)).toBe("10.0.0.2");
    expect(trustedClientIp(req, "10.0.0.2", "")).toBe("10.0.0.2");
  });

  it("ignores cf-connecting-ip without the edge marker", () => {
    const req = request({ "cf-connecting-ip": "198.51.100.7" });
    expect(trustedClientIp(req, "10.0.0.2", MARKER)).toBe("10.0.0.2");
  });

  it("ignores cf-connecting-ip when the marker is wrong", () => {
    const req = request({
      "cf-connecting-ip": "198.51.100.7",
      "x-eq-ingress-marker": "b".repeat(64),
    });
    expect(trustedClientIp(req, "10.0.0.2", MARKER)).toBe("10.0.0.2");
  });

  it("believes cf-connecting-ip only with the matching marker", () => {
    const req = request({
      "cf-connecting-ip": "198.51.100.7",
      "x-eq-ingress-marker": MARKER,
    });
    expect(trustedClientIp(req, "10.0.0.2", MARKER)).toBe("198.51.100.7");
  });

  it("falls back to the peer when marked but cf-connecting-ip is absent", () => {
    const req = request({ "x-eq-ingress-marker": MARKER });
    expect(trustedClientIp(req, "10.0.0.2", MARKER)).toBe("10.0.0.2");
  });
});
