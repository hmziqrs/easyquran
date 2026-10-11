import { describe, expect, it, vi } from "vitest";
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import { getAuthCopy } from "#lib/i18n/auth-copy.js";

import {
  ACCOUNT_PATH,
  ANONYMOUS_CTA_HREF,
  resolveAccountView,
  resolveLinkNotice,
  sessionDeviceLabel,
} from "../account-view";

const copy = getAuthCopy("en");

function params(query: string): URLSearchParams {
  return new URLSearchParams(query);
}

describe("resolveAccountView hydration branches", () => {
  it("renders the neutral loading shell while status is unknown (prerender-safe: no user data)", () => {
    expect(resolveAccountView("unknown")).toBe("loading");
  });

  it("renders the login CTA when anonymous", () => {
    expect(resolveAccountView("anonymous")).toBe("anonymous");
  });

  it("renders profile + sessions + security when authenticated", () => {
    expect(resolveAccountView("authenticated")).toBe("authenticated");
  });

  it("never treats unknown as anonymous (no CTA flash before probe resolves)", () => {
    expect(resolveAccountView("unknown")).not.toBe("anonymous");
    expect(resolveAccountView("unknown")).not.toBe("authenticated");
  });
});

describe("account anonymous CTA target round-trips to /account after login", () => {
  it("points the CTA at /login (internal, allowlisted)", () => {
    expect(ANONYMOUS_CTA_HREF.startsWith("/")).toBe(true);
    expect(ANONYMOUS_CTA_HREF.startsWith("//")).toBe(false);
  });

  it("ACCOUNT_PATH is the internal same-origin account route", () => {
    expect(ACCOUNT_PATH).toBe("/account");
    expect(ACCOUNT_PATH.startsWith("//")).toBe(false);
  });
});

describe("resolveLinkNotice maps link-flow redirect params", () => {
  it("returns null without link params", () => {
    expect(resolveLinkNotice(params(""), copy, null)).toBeNull();
    expect(resolveLinkNotice(params("tab=security"), copy, null)).toBeNull();
  });

  it("success banner names the linked provider", () => {
    expect(resolveLinkNotice(params("linked=github"), copy, null)).toEqual({
      kind: "success",
      text: copy.connectedLinkedBanner("GitHub"),
    });
  });

  it("ignores an unknown linked provider", () => {
    expect(resolveLinkNotice(params("linked=steam"), copy, null)).toBeNull();
  });

  it("cancelled maps to the cancelled copy", () => {
    expect(resolveLinkNotice(params("link_error=cancelled"), copy, null)).toEqual({
      kind: "error",
      text: copy.connectedCancelledError,
    });
  });

  it("link_failed maps to the generic retry copy", () => {
    expect(resolveLinkNotice(params("link_error=link_failed"), copy, null)).toEqual({
      kind: "error",
      text: copy.connectedFailedError,
    });
  });

  it("already_linked names the attempted provider when it is known", () => {
    expect(resolveLinkNotice(params("link_error=already_linked"), copy, "discord")).toEqual({
      kind: "error",
      text: copy.connectedAlreadyLinkedError("Discord"),
    });
  });

  it("already_linked without a remembered provider falls back to the generic copy", () => {
    expect(resolveLinkNotice(params("link_error=already_linked"), copy, null)).toEqual({
      kind: "error",
      text: copy.connectedFailedError,
    });
  });
});

describe("sessionDeviceLabel never leaks raw UA detail", () => {
  it("returns a generic label for missing UA", () => {
    expect(sessionDeviceLabel(undefined)).toBe("This device");
    expect(sessionDeviceLabel("")).toBe("This device");
  });

  it("coarse-classifies mobile/tablet without echoing versions or identifiers", () => {
    const mobile = sessionDeviceLabel(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) user123@eq.test",
    );
    expect(mobile).toBe("Mobile device");
    expect(mobile).not.toContain("@");
    expect(mobile).not.toContain("17_2");

    const tablet = sessionDeviceLabel("Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X)");
    expect(tablet).toBe("Tablet");

    const desktop = sessionDeviceLabel(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
    );
    expect(desktop).toBe("This device");
    expect(desktop).not.toContain("537.36");
  });
});
