import { describe, expect, it } from "vite-plus/test";

import { authFooterLinkIds, authPageTitle } from "#lib/auth/auth-page-meta.js";

describe("authFooterLinkIds", () => {
  it("shows the legal pair on account-creation pages", () => {
    expect(authFooterLinkIds("/login")).toEqual(["privacy", "terms"]);
    expect(authFooterLinkIds("/register")).toEqual(["privacy", "terms"]);
  });

  it("shows the support pair on recovery pages", () => {
    expect(authFooterLinkIds("/forgot-password")).toEqual(["contact", "faq"]);
    expect(authFooterLinkIds("/verify-email")).toEqual(["contact", "faq"]);
  });

  it("normalizes trailing slashes", () => {
    expect(authFooterLinkIds("/login/")).toEqual(["privacy", "terms"]);
  });

  it("returns no links for other paths", () => {
    expect(authFooterLinkIds("/")).toEqual([]);
    expect(authFooterLinkIds("/account")).toEqual([]);
    expect(authFooterLinkIds("/ar/login")).toEqual([]);
  });
});

describe("authPageTitle", () => {
  it("resolves every auth form page title", () => {
    expect(authPageTitle("/login")).toBe("Sign in · EasyQuran");
    expect(authPageTitle("/register")).toBe("Create account · EasyQuran");
    expect(authPageTitle("/forgot-password")).toBe("Reset your password · EasyQuran");
    expect(authPageTitle("/verify-email")).toBe("Verify your email · EasyQuran");
  });

  it("normalizes trailing slashes", () => {
    expect(authPageTitle("/register/")).toBe("Create account · EasyQuran");
  });

  it("returns null for other paths", () => {
    expect(authPageTitle("/")).toBeNull();
    expect(authPageTitle("/account")).toBeNull();
  });
});
