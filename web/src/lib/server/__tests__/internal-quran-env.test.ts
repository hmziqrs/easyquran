import { describe, expect, it } from "vite-plus/test";

import { assertInternalQuranEnv } from "#lib/server/internal-quran-env.js";

const FULL = {
  INTERNAL_QURAN_API_BASE: "http://api:8888/quran",
  INTERNAL_QURAN_API_TOKEN: "x".repeat(32),
};

describe("assertInternalQuranEnv", () => {
  it("refuses production boot without the internal base", () => {
    expect(() =>
      assertInternalQuranEnv({
        NODE_ENV: "production",
        INTERNAL_QURAN_API_TOKEN: FULL.INTERNAL_QURAN_API_TOKEN,
      }),
    ).toThrow(/INTERNAL_QURAN_API_BASE/u);
  });

  it("refuses production boot without the internal token", () => {
    expect(() =>
      assertInternalQuranEnv({
        NODE_ENV: "production",
        INTERNAL_QURAN_API_BASE: FULL.INTERNAL_QURAN_API_BASE,
      }),
    ).toThrow(/INTERNAL_QURAN_API_TOKEN/u);
  });

  it("names both when both are missing", () => {
    expect(() => assertInternalQuranEnv({ NODE_ENV: "production" })).toThrow(
      /INTERNAL_QURAN_API_BASE, INTERNAL_QURAN_API_TOKEN/u,
    );
  });

  it("treats blank values as missing", () => {
    expect(() =>
      assertInternalQuranEnv({
        NODE_ENV: "production",
        INTERNAL_QURAN_API_BASE: "  ",
        INTERNAL_QURAN_API_TOKEN: FULL.INTERNAL_QURAN_API_TOKEN,
      }),
    ).toThrow(/INTERNAL_QURAN_API_BASE/u);
  });

  it("boots production with the full pair", () => {
    expect(() => assertInternalQuranEnv({ NODE_ENV: "production", ...FULL })).not.toThrow();
  });

  it("does not gate non-production environments", () => {
    expect(() => assertInternalQuranEnv({ NODE_ENV: "development" })).not.toThrow();
    expect(() => assertInternalQuranEnv({})).not.toThrow();
  });
});
