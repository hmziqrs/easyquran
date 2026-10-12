import { describe, expect, it, vi } from "vite-plus/test";
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
  sessionLastSeenLabel,
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

describe("sessionDeviceLabel parses the UA into a browser · OS label", () => {
  it("returns Unknown device for a missing UA", () => {
    expect(sessionDeviceLabel(undefined)).toBe("Unknown device");
    expect(sessionDeviceLabel("")).toBe("Unknown device");
  });

  it("names browser and OS for common desktop UAs", () => {
    const macChrome =
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    expect(sessionDeviceLabel(macChrome)).toBe("Chrome · macOS");

    const windowsFirefox =
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0";
    expect(sessionDeviceLabel(windowsFirefox)).toBe("Firefox · Windows");

    const linuxChrome =
      "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    expect(sessionDeviceLabel(linuxChrome)).toBe("Chrome · Linux");
  });

  it("classifies mobile UAs without echoing versions or identifiers", () => {
    const mobile =
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Mobile/15E148 Safari/604.1 user123@eq.test";
    const mobileLabel = sessionDeviceLabel(mobile);
    expect(mobileLabel).toBe("Safari · iOS");
    expect(mobileLabel).not.toContain("@");
    expect(mobileLabel).not.toContain("17_2");

    const tablet =
      "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15";
    expect(sessionDeviceLabel(tablet)).toBe("Safari · iPadOS");

    const android =
      "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36";
    expect(sessionDeviceLabel(android)).toBe("Chrome · Android");
  });

  it("matches Chromium forks before the generic Chrome/Safari tokens their UAs carry", () => {
    const edge =
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
    expect(sessionDeviceLabel(edge)).toBe("Edge · Windows");

    const opera =
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 OPR/110.0.0.0";
    expect(sessionDeviceLabel(opera)).toBe("Opera · macOS");
  });

  it("never echoes an unparseable UA", () => {
    const label = sessionDeviceLabel("some-scanner/1.2 (user@host)");
    expect(label).toBe("Unknown device");
    expect(label).not.toContain("@");
  });
});

describe("sessionLastSeenLabel formats last_seen for humans", () => {
  const ISO = "2026-10-12T07:47:12.919710Z";

  it("formats locale-aware date + time, never raw ISO text", () => {
    const label = sessionLastSeenLabel(ISO, "en");
    const expected = new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    }).format(Date.parse(ISO));
    expect(label).toBe(expected);
    expect(label).not.toContain("Z");
    expect(label).not.toContain("919710");
  });

  it("survives sub-millisecond precision the engine may refuse to parse", () => {
    expect(sessionLastSeenLabel(ISO, "en")).not.toBeNull();
    expect(sessionLastSeenLabel("2026-10-12T07:47:12.919Z", "en")).not.toBeNull();
  });

  it("returns null for missing or unparsable stamps (row hides the line)", () => {
    expect(sessionLastSeenLabel("", "en")).toBeNull();
    expect(sessionLastSeenLabel("not-a-date", "en")).toBeNull();
  });

  it("follows the page locale", () => {
    const en = sessionLastSeenLabel(ISO, "en");
    const ar = sessionLastSeenLabel(ISO, "ar");
    expect(ar).toBeTruthy();
    expect(ar).not.toBe(en);
  });
});
