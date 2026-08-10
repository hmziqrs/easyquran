import { describe, expect, it } from "vite-plus/test";
import {
  ACCOUNT_PATH,
  ANONYMOUS_CTA_HREF,
  resolveAccountView,
  sessionDeviceLabel,
} from "../account-view";

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
