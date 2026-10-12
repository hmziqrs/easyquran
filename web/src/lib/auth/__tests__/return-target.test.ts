import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  appPathFromReferrer,
  clearReturnTarget,
  consumeReturnTarget,
  currentAppReturnPath,
  getReturnTarget,
  isInternalPath,
  setReturnTarget,
} from "#lib/auth/return-target.js";

afterEach(() => {
  sessionStorage.clear();
  history.replaceState(null, "", "/");
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
    setReturnTarget("/surah");
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

describe("isInternalPath", () => {
  it("accepts app paths with query and hash", () => {
    expect(isInternalPath("/bookmarks")).toBe(true);
    expect(isInternalPath("/t/2?foo=bar")).toBe(true);
  });

  it("rejects external, protocol-relative, and junk values", () => {
    expect(isInternalPath(null)).toBe(false);
    expect(isInternalPath("")).toBe(false);
    expect(isInternalPath("https://evil.example")).toBe(false);
    expect(isInternalPath("//evil.example")).toBe(false);
    expect(isInternalPath("/\\evil.example")).toBe(false);
    expect(isInternalPath("/has space")).toBe(false);
  });
});

describe("currentAppReturnPath", () => {
  it("returns the current path with query on an app route", () => {
    history.replaceState(null, "", "/bookmarks?filter=all");
    expect(currentAppReturnPath()).toBe("/bookmarks?filter=all");
  });

  it("returns null on the sign-in page and other auth surfaces", () => {
    history.replaceState(null, "", "/login?returnTo=/bookmarks");
    expect(currentAppReturnPath()).toBeNull();
    history.replaceState(null, "", "/verify-email");
    expect(currentAppReturnPath()).toBeNull();
    history.replaceState(null, "", "/auth/google/success");
    expect(currentAppReturnPath()).toBeNull();
  });

  it("a target stored on an auth surface survives show-there (currentAppReturnPath is null, not a clobber)", () => {
    setReturnTarget("/bookmarks");
    history.replaceState(null, "", "/login");
    expect(currentAppReturnPath()).toBeNull();
    expect(getReturnTarget()).toBe("/bookmarks");
  });
});

describe("appPathFromReferrer", () => {
  it("accepts a same-origin app path with query", () => {
    const ref = `${location.origin}/surah/1?t=saheeh`;
    expect(appPathFromReferrer(ref)).toBe("/surah/1?t=saheeh");
  });

  it("rejects cross-origin referrers", () => {
    expect(appPathFromReferrer("https://evil.example/bookmarks")).toBeNull();
  });

  it("rejects auth-surface referrers", () => {
    expect(appPathFromReferrer(`${location.origin}/login`)).toBeNull();
    expect(appPathFromReferrer(`${location.origin}/register`)).toBeNull();
    expect(appPathFromReferrer(`${location.origin}/auth/google/failure`)).toBeNull();
  });

  it("rejects missing and unparseable referrers", () => {
    expect(appPathFromReferrer(null)).toBeNull();
    expect(appPathFromReferrer("")).toBeNull();
    expect(appPathFromReferrer("not a url")).toBeNull();
  });
});
