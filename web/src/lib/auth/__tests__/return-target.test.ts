import {
  clearReturnTarget,
  consumeReturnTarget,
  getReturnTarget,
  setReturnTarget,
} from "$lib/auth/return-target";
import { afterEach, describe, expect, it } from "vite-plus/test";

afterEach(() => {
  sessionStorage.clear();
});

describe("return-target set/get", () => {
  it("round-trips an internal path", () => {
    expect(setReturnTarget("/account/security")).toBe(true);
    expect(getReturnTarget()).toBe("/account/security");
  });

  it("accepts a path with query/hash (still same-origin internal)", () => {
    expect(setReturnTarget("/account?tab=sessions#top")).toBe(true);
    expect(getReturnTarget()).toBe("/account?tab=sessions#top");
  });

  it("returns null when nothing stored", () => {
    expect(getReturnTarget()).toBeNull();
  });
});

describe("return-target reject non-internal / sensitive values", () => {
  it("rejects external URLs", () => {
    expect(setReturnTarget("https://evil.example/steal")).toBe(false);
    expect(getReturnTarget()).toBeNull();
  });

  it("rejects protocol-relative URLs", () => {
    expect(setReturnTarget("//evil.example")).toBe(false);
    expect(getReturnTarget()).toBeNull();
  });

  it("rejects backslash schemes", () => {
    expect(setReturnTarget("/\\evil.example")).toBe(false);
  });

  it("rejects empty / whitespace values", () => {
    expect(setReturnTarget("")).toBe(false);
    expect(setReturnTarget("   /account")).toBe(false);
    expect(getReturnTarget()).toBeNull();
  });

  it("never stores tokens or emails (only internal paths accepted)", () => {
    expect(setReturnTarget("code=abc&email=a@b.c")).toBe(false);
    expect(getReturnTarget()).toBeNull();
  });
});

describe("return-target consume-once", () => {
  it("consume returns the stored path then clears", () => {
    setReturnTarget("/reader/2");
    expect(consumeReturnTarget()).toBe("/reader/2");
    expect(getReturnTarget()).toBeNull();
    expect(consumeReturnTarget()).toBeNull();
  });

  it("a second consume after success returns null", () => {
    setReturnTarget("/app");
    consumeReturnTarget();
    expect(consumeReturnTarget()).toBeNull();
  });
});

describe("return-target clear", () => {
  it("clear removes a stored target", () => {
    setReturnTarget("/x");
    clearReturnTarget();
    expect(getReturnTarget()).toBeNull();
  });

  it("clear is a no-op when nothing is stored", () => {
    expect(() => clearReturnTarget()).not.toThrow();
    expect(getReturnTarget()).toBeNull();
  });

  it("clears after failure too (garbage in storage is purged on read)", () => {
    sessionStorage.setItem("eq:oauth-return", "https://garbage.example");
    expect(getReturnTarget()).toBeNull();
    expect(sessionStorage.getItem("eq:oauth-return")).toBeNull();
  });
});
