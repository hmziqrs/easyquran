import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));

import {
  awaitOAuthPopup,
  isOAuthPopupMessage,
  notifyOAuthPopupOpener,
  oauthPopupFeatures,
  OAUTH_POPUP_SOURCE,
  type OAuthPopupHandle,
  type OAuthPopupMessage,
} from "#lib/auth/oauth-popup.js";

class FakePopup implements OAuthPopupHandle {
  closed = false;
  focus = vi.fn();
  close(): void {
    this.closed = true;
  }
}

function popupMessage(provider: string, outcome: "success" | "failure"): OAuthPopupMessage {
  return { source: OAUTH_POPUP_SOURCE, provider, outcome };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- test helper deliberately forwards malformed payloads into the parser boundary
function dispatchMessage(data: unknown, origin: string = window.location.origin): void {
  window.dispatchEvent(new MessageEvent("message", { data, origin }));
}

afterEach(() => {
  Object.defineProperty(window, "opener", { value: null, configurable: true });
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("isOAuthPopupMessage", () => {
  it("accepts the exact popup protocol shape", () => {
    expect(isOAuthPopupMessage(popupMessage("google", "success"))).toBe(true);
    expect(isOAuthPopupMessage(popupMessage("github", "failure"))).toBe(true);
  });

  it("rejects foreign or malformed payloads", () => {
    expect(isOAuthPopupMessage(null)).toBe(false);
    expect(isOAuthPopupMessage("eq:oauth-popup")).toBe(false);
    expect(isOAuthPopupMessage({ source: "other", provider: "google", outcome: "success" })).toBe(
      false,
    );
    expect(
      isOAuthPopupMessage({ source: OAUTH_POPUP_SOURCE, provider: 7, outcome: "success" }),
    ).toBe(false);
    expect(isOAuthPopupMessage({ source: OAUTH_POPUP_SOURCE, provider: "google" })).toBe(false);
    expect(
      isOAuthPopupMessage({ source: OAUTH_POPUP_SOURCE, provider: "google", outcome: "maybe" }),
    ).toBe(false);
  });
});

describe("awaitOAuthPopup", () => {
  it("opens the provider popup at the login URL and resolves completed on success", async () => {
    const popup = new FakePopup();
    const open = vi.fn().mockReturnValue(popup);
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/api/auth/google/v1/login",
      open,
      features: "popup=yes",
    });

    expect(open).toHaveBeenCalledWith("/api/auth/google/v1/login", "eq-oauth-google", "popup=yes");
    expect(popup.focus).toHaveBeenCalled();
    dispatchMessage(popupMessage("google", "success"));
    await expect(outcome).resolves.toBe("completed");
  });

  it("resolves failed on a failure report", async () => {
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
    });
    dispatchMessage(popupMessage("google", "failure"));
    await expect(outcome).resolves.toBe("failed");
  });

  it("resolves blocked when the popup never opens", async () => {
    const outcome = awaitOAuthPopup({ provider: "google", url: "/login", open: () => null });
    await expect(outcome).resolves.toBe("blocked");
  });

  it("resolves cancelled when the popup closes without a message", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
    });
    popup.close();
    await vi.advanceTimersByTimeAsync(50);
    await expect(outcome).resolves.toBe("cancelled");
  });

  it("ignores messages from another origin", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
    });
    dispatchMessage(popupMessage("google", "success"), "https://evil.test");
    popup.close();
    await vi.advanceTimersByTimeAsync(50);
    await expect(outcome).resolves.toBe("cancelled");
  });

  it("ignores messages for another provider", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
    });
    dispatchMessage(popupMessage("github", "success"));
    popup.close();
    await vi.advanceTimersByTimeAsync(50);
    await expect(outcome).resolves.toBe("cancelled");
  });

  it("a message landing inside the close grace still wins", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 30,
    });
    popup.close();
    await vi.advanceTimersByTimeAsync(10);
    dispatchMessage(popupMessage("google", "success"));
    await expect(outcome).resolves.toBe("completed");
  });
});

describe("notifyOAuthPopupOpener", () => {
  it("returns false when there is no opener", () => {
    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
  });

  it("posts the completion signal to the same-origin opener and closes the popup", () => {
    const postMessage = vi.fn();
    Object.defineProperty(window, "opener", { value: { postMessage }, configurable: true });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(true);
    expect(postMessage).toHaveBeenCalledWith(
      { source: OAUTH_POPUP_SOURCE, provider: "google", outcome: "success" },
      window.location.origin,
    );
    expect(close).toHaveBeenCalled();
  });
});

describe("oauthPopupFeatures", () => {
  it("requests a centered popup sized for the provider page", () => {
    const features = oauthPopupFeatures();
    expect(features).toContain("popup=yes");
    expect(features).toContain("width=480");
    expect(features).toContain("height=640");
  });
});
