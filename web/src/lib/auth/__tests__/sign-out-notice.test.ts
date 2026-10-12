import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import {
  armSignOutNotice,
  clearSignOutNotice,
  consumeSignOutNotice,
  signOutNoticeArmed,
} from "#lib/auth/sign-out-notice.svelte.js";

/** Counts per domain — stands in for the engine's durable pending counters. */
function counter(domains: Record<string, number>): (domain: string) => number {
  return (domain) => domains[domain] ?? 0;
}

beforeEach(() => {
  clearSignOutNotice();
});

describe("sign-out notice state (§9.5, EC-14)", () => {
  it("arms when either sync partition of the signing-out account has queued work", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 2 }));
    expect(signOutNoticeArmed()).toBe(true);
    expect(consumeSignOutNotice()).toBe(true);

    armSignOutNotice(7, counter({ "sessions.user:7": 1 }));
    expect(consumeSignOutNotice()).toBe(true);
  });

  it("stays disarmed when both partitions are empty", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 0, "sessions.user:7": 0 }));
    expect(signOutNoticeArmed()).toBe(false);
  });

  it("never counts another account's quarantined partition", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:8": 5, "sessions.user:8": 5 }));
    expect(signOutNoticeArmed()).toBe(false);
  });

  it("a null owner (already signed out) disarms", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 1 }));
    armSignOutNotice(null, counter({}));
    expect(signOutNoticeArmed()).toBe(false);
  });

  it("consume is one-shot: a second read sees the cleared state", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 1 }));
    expect(consumeSignOutNotice()).toBe(true);
    expect(consumeSignOutNotice()).toBe(false);
  });

  it("a later sign-in edge invalidates an unread notice", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 1 }));
    clearSignOutNotice();
    expect(consumeSignOutNotice()).toBe(false);
  });

  it("re-arming overwrites: a clean sign-out clears a stale armed notice", () => {
    armSignOutNotice(7, counter({ "bookmarks.user:7": 1 }));
    armSignOutNotice(7, counter({ "bookmarks.user:7": 0, "sessions.user:7": 0 }));
    expect(signOutNoticeArmed()).toBe(false);
  });
});
