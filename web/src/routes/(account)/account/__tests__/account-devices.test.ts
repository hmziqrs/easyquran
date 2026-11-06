import { describe, expect, it, vi } from "vite-plus/test";
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import type { DeviceInfo } from "#lib/auth/account-client.js";

import {
  deviceIconName,
  deviceTitle,
  deviceTypeLabel,
  normaliseDeviceNameDraft,
} from "../account-view";

function device(overrides: Partial<DeviceInfo>): DeviceInfo {
  return {
    id: "dev-1",
    displayName: null,
    platform: "web",
    client: "web",
    formFactor: "desktop",
    browser: "chrome",
    osMajor: null,
    isCurrent: false,
    ...overrides,
  };
}

describe("deviceTypeLabel uses fixed tables keyed by the closed server enums", () => {
  it("names the browser for the web client and the platform for native clients", () => {
    expect(deviceTypeLabel(device({ browser: "firefox" }))).toBe("Firefox");
    expect(
      deviceTypeLabel(device({ platform: "ios", client: "react_native", browser: null })),
    ).toBe("iPhone");
    expect(
      deviceTypeLabel(
        device({ platform: "ios", client: "react_native", browser: null, formFactor: "tablet" }),
      ),
    ).toBe("iPad");
    expect(
      deviceTypeLabel(device({ platform: "android", client: "react_native", browser: null })),
    ).toBe("Android phone");
    expect(deviceTypeLabel(device({ platform: "macos", client: "gpui", browser: null }))).toBe(
      "Mac",
    );
  });

  it("falls back to the unknown label for values outside the tables and never echoes them", () => {
    expect(deviceTypeLabel(device({ browser: "<script>x</script>" }), "Unknown device")).toBe(
      "Unknown device",
    );
    expect(deviceTypeLabel(device({ browser: null }), "Unknown device")).toBe("Unknown device");
    expect(deviceTypeLabel(device({ platform: "plan9" }), "Unknown device")).toBe("Unknown device");
  });
});

describe("deviceTitle prefers the user's name", () => {
  it("shows the name when one is set, the type label otherwise", () => {
    expect(deviceTitle(device({ displayName: "Work laptop" }))).toBe("Work laptop");
    expect(deviceTitle(device({}))).toBe("Chrome");
  });
});

describe("deviceIconName", () => {
  it("draws a phone for iOS and Android and a monitor for everything else", () => {
    expect(deviceIconName(device({ platform: "ios" }))).toBe("device-phone");
    expect(deviceIconName(device({ platform: "android" }))).toBe("device-phone");
    expect(deviceIconName(device({ platform: "web" }))).toBe("device-monitor");
    expect(deviceIconName(device({ platform: "linux" }))).toBe("device-monitor");
  });
});

describe("normaliseDeviceNameDraft mirrors the server's name rule", () => {
  it("trims and accepts 1 to 40 characters", () => {
    expect(normaliseDeviceNameDraft("  Pixel 8  ")).toBe("Pixel 8");
    expect(normaliseDeviceNameDraft("a".repeat(40))).toBe("a".repeat(40));
  });

  it("refuses an empty name and one past 40 characters", () => {
    expect(normaliseDeviceNameDraft("   ")).toBeNull();
    expect(normaliseDeviceNameDraft("a".repeat(41))).toBeNull();
  });

  it("counts code points, not UTF-16 units", () => {
    expect(normaliseDeviceNameDraft("😀".repeat(40))).toBe("😀".repeat(40));
    expect(normaliseDeviceNameDraft("😀".repeat(41))).toBeNull();
  });
});
