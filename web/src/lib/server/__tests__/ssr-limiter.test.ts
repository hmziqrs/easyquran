import { describe, expect, it } from "vite-plus/test";

import {
  hasLoggedInMarker,
  isHealthPath,
  isTranslatedReaderPath,
  parseSsrLimitConfig,
  SsrMissLimiter,
} from "#lib/server/ssr-limiter.js";

function clock(start = 1_000_000) {
  let now = start;
  return {
    now: () => now,
    advance: (ms: number) => {
      now += ms;
    },
  };
}

describe("parseSsrLimitConfig", () => {
  it("is fully off when no ceiling env is set", () => {
    expect(parseSsrLimitConfig({})).toEqual({
      perIp: null,
      perSharedIp: null,
      sharedIpPrefixes: [],
      global: null,
    });
  });

  it("parses ceilings and shared prefixes", () => {
    const config = parseSsrLimitConfig({
      WEB_SSR_IP_PER_MINUTE: "120",
      WEB_SSR_SHARED_IP_PER_MINUTE: " 30 ",
      WEB_SSR_SHARED_IP_PREFIXES: "100.64.,198.18. ",
      WEB_SSR_GLOBAL_PER_MINUTE: "500",
    });
    expect(config).toEqual({
      perIp: 120,
      perSharedIp: 30,
      sharedIpPrefixes: ["100.64.", "198.18."],
      global: 500,
    });
  });

  it("refuses a malformed or zero ceiling at boot", () => {
    expect(() => parseSsrLimitConfig({ WEB_SSR_IP_PER_MINUTE: "0" })).toThrow(
      /WEB_SSR_IP_PER_MINUTE/u,
    );
    expect(() => parseSsrLimitConfig({ WEB_SSR_GLOBAL_PER_MINUTE: "ten" })).toThrow(
      /WEB_SSR_GLOBAL_PER_MINUTE/u,
    );
  });
});

describe("path predicates", () => {
  it("matches translated reader routes only", () => {
    expect(isTranslatedReaderPath("/t/en/sahih/page/3")).toBe(true);
    expect(isTranslatedReaderPath("/ar/t/en/sahih/juz/2")).toBe(true);
    expect(isTranslatedReaderPath("/2/t/en/sahih")).toBe(true);
    expect(isTranslatedReaderPath("/")).toBe(false);
    expect(isTranslatedReaderPath("/page/3")).toBe(false);
  });

  it("exempts health paths", () => {
    expect(isHealthPath("/healthz")).toBe(true);
    expect(isHealthPath("/quran/health/ready")).toBe(true);
    expect(isHealthPath("/t/en/sahih/page/3")).toBe(false);
  });

  it("requires both the marker and a session cookie for logged-in", () => {
    expect(hasLoggedInMarker("ruxlog.auth=1; ruxlog.sid=abc")).toBe(true);
    expect(hasLoggedInMarker("_ga=GA1.2; ruxlog.auth=1")).toBe(false);
    expect(hasLoggedInMarker("ruxlog.sid=abc")).toBe(false);
    expect(hasLoggedInMarker(null)).toBe(false);
  });
});

describe("SsrMissLimiter", () => {
  it("never blocks while every ceiling is off", () => {
    const limiter = new SsrMissLimiter(parseSsrLimitConfig({}), clock().now);
    for (let i = 0; i < 1000; i += 1) expect(limiter.check("203.0.113.1", false)).toBeNull();
  });

  it("blocks one IP past its ceiling without touching another IP", () => {
    const limiter = new SsrMissLimiter(
      parseSsrLimitConfig({ WEB_SSR_IP_PER_MINUTE: "2" }),
      clock().now,
    );
    expect(limiter.check("203.0.113.1", false)).toBeNull();
    expect(limiter.check("203.0.113.1", false)).toBeNull();
    const blocked = limiter.check("203.0.113.1", false);
    expect(blocked).not.toBeNull();
    expect(blocked?.retryAfterSecs).toBeGreaterThanOrEqual(1);
    expect(limiter.check("203.0.113.2", false)).toBeNull();
  });

  it("gives shared addresses the lower class", () => {
    const limiter = new SsrMissLimiter(
      parseSsrLimitConfig({
        WEB_SSR_IP_PER_MINUTE: "10",
        WEB_SSR_SHARED_IP_PER_MINUTE: "1",
        WEB_SSR_SHARED_IP_PREFIXES: "100.64.",
      }),
      clock().now,
    );
    expect(limiter.check("100.64.0.9", false)).toBeNull();
    expect(limiter.check("100.64.0.9", false)).not.toBeNull();
    expect(limiter.check("203.0.113.9", false)).toBeNull();
    expect(limiter.check("203.0.113.9", false)).toBeNull();
  });

  it("exempts logged-in readers from the per-IP class but not the global backstop", () => {
    const limiter = new SsrMissLimiter(
      parseSsrLimitConfig({ WEB_SSR_IP_PER_MINUTE: "1", WEB_SSR_GLOBAL_PER_MINUTE: "3" }),
      clock().now,
    );
    expect(limiter.check("203.0.113.5", true)).toBeNull();
    expect(limiter.check("203.0.113.5", true)).toBeNull();
    expect(limiter.check("203.0.113.5", true)).toBeNull();
    expect(limiter.check("203.0.113.5", true)).not.toBeNull();
  });

  it("resets after the one-minute window", () => {
    const time = clock();
    const limiter = new SsrMissLimiter(
      parseSsrLimitConfig({ WEB_SSR_IP_PER_MINUTE: "1" }),
      time.now,
    );
    expect(limiter.check("203.0.113.7", false)).toBeNull();
    expect(limiter.check("203.0.113.7", false)).not.toBeNull();
    time.advance(60_000);
    expect(limiter.check("203.0.113.7", false)).toBeNull();
  });
});
