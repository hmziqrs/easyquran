import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true }));

import {
  awaitOAuthPopup,
  clearOAuthPopupPending,
  isOAuthPopupMessage,
  markOAuthPopupPending,
  notifyOAuthPopupOpener,
  oauthPopupFeatures,
  oauthPopupName,
  OAUTH_POPUP_NAME_PREFIX,
  OAUTH_POPUP_PENDING_KEY,
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

interface OpenerStub {
  readonly location?: { readonly origin?: string };
  postMessage?: (message: OAuthPopupMessage, targetOrigin: string) => void;
  readonly sessionStorage?: { getItem(key: string): string | null };
}

/* eslint-disable anti-slop/no-unknown-parameters -- test helper deliberately forwards malformed payloads into the parser boundary */
function dispatchMessage(
  data: unknown,
  source: OAuthPopupHandle | null = null,
  origin: string = window.location.origin,
): void {
  const event = new MessageEvent("message", { data, origin });
  Object.defineProperty(event, "source", { value: source });
  window.dispatchEvent(event);
}
/* eslint-enable anti-slop/no-unknown-parameters */

function setOpener(value: OpenerStub | null): void {
  Object.defineProperty(window, "opener", { value, configurable: true });
}

afterEach(() => {
  setOpener(null);
  window.name = "";
  sessionStorage.clear();
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
  it("opens a uniquely named popup at the login URL and resolves completed on success", async () => {
    const popup = new FakePopup();
    const open = vi.fn().mockReturnValue(popup);
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/api/auth/google/v1/login",
      open,
      features: "popup=yes",
    });

    expect(open).toHaveBeenCalledWith(
      "/api/auth/google/v1/login",
      expect.stringMatching(/^eq-oauth-google-/),
      "popup=yes",
    );
    expect(popup.focus).toHaveBeenCalled();
    dispatchMessage(popupMessage("google", "success"), popup);
    await expect(outcome).resolves.toBe("completed");
  });

  it("waits on an already-open popup without opening another", async () => {
    const popup = new FakePopup();
    const open = vi.fn();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open,
      popup,
      features: "",
    });

    expect(open).not.toHaveBeenCalled();
    dispatchMessage(popupMessage("google", "success"), popup);
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
    dispatchMessage(popupMessage("google", "failure"), popup);
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
    dispatchMessage(popupMessage("google", "success"), popup, "https://evil.test");
    popup.close();
    await vi.advanceTimersByTimeAsync(50);
    await expect(outcome).resolves.toBe("cancelled");
  });

  it("ignores messages from a window this attempt did not open", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const other = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
    });
    dispatchMessage(popupMessage("google", "success"), other);
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
    dispatchMessage(popupMessage("github", "success"), popup);
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
    dispatchMessage(popupMessage("google", "success"), popup);
    await expect(outcome).resolves.toBe("completed");
  });

  it("removes the listener and timers on settle; later messages are ignored", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const removeListener = vi.spyOn(window, "removeEventListener");
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
    });
    dispatchMessage(popupMessage("google", "success"), popup);
    await expect(outcome).resolves.toBe("completed");
    expect(removeListener).toHaveBeenCalledWith("message", expect.any(Function));
    expect(vi.getTimerCount()).toBe(0);

    dispatchMessage(popupMessage("google", "failure"), popup);
    await vi.advanceTimersByTimeAsync(50);
    await expect(outcome).resolves.toBe("completed");
  });

  it("settles unavailable at the deadline and closes the orphan popup", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
      deadlineMs: 50,
    });
    await vi.advanceTimersByTimeAsync(60);
    await expect(outcome).resolves.toBe("unavailable");
    expect(popup.closed).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("still settles unavailable at the deadline when the popup close throws", async () => {
    vi.useFakeTimers();
    const popup = new FakePopup();
    popup.close = vi.fn(() => {
      throw new Error("close refused");
    });
    const outcome = awaitOAuthPopup({
      provider: "google",
      url: "/login",
      open: () => popup,
      features: "",
      pollMs: 10,
      closeGraceMs: 5,
      deadlineMs: 50,
    });
    await vi.advanceTimersByTimeAsync(60);
    await expect(outcome).resolves.toBe("unavailable");
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe("notifyOAuthPopupOpener", () => {
  it("returns false when there is no opener", () => {
    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
  });

  it("posts the completion signal to a same-origin opener with a pending marker and closes the popup", () => {
    sessionStorage.setItem(OAUTH_POPUP_PENDING_KEY, "1");
    const postMessage = vi.fn();
    setOpener({ location: { origin: window.location.origin }, postMessage, sessionStorage });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(true);
    expect(postMessage).toHaveBeenCalledWith(
      { source: OAUTH_POPUP_SOURCE, provider: "google", outcome: "success" },
      window.location.origin,
    );
    expect(close).toHaveBeenCalled();
  });

  it("ignores window.name — a pending marker still completes with a replaced name", () => {
    window.name = "some-other-window";
    sessionStorage.setItem(OAUTH_POPUP_PENDING_KEY, "1");
    const postMessage = vi.fn();
    setOpener({ location: { origin: window.location.origin }, postMessage, sessionStorage });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(true);
    expect(postMessage).toHaveBeenCalled();
    expect(close).toHaveBeenCalled();
  });

  it("refuses a same-origin opener with no pending marker without posting or closing", () => {
    const postMessage = vi.fn();
    setOpener({ location: { origin: window.location.origin }, postMessage, sessionStorage });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
    expect(postMessage).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it("refuses when the opener's pending marker was already settled (cleared)", () => {
    const postMessage = vi.fn();
    setOpener({ location: { origin: window.location.origin }, postMessage, sessionStorage });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});
    sessionStorage.setItem(OAUTH_POPUP_PENDING_KEY, "1");
    sessionStorage.removeItem(OAUTH_POPUP_PENDING_KEY);

    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
    expect(postMessage).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it("refuses a foreign-origin opener without posting or closing", () => {
    sessionStorage.setItem(OAUTH_POPUP_PENDING_KEY, "1");
    const postMessage = vi.fn();
    setOpener({ location: { origin: "https://evil.test" }, postMessage, sessionStorage });
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
    expect(postMessage).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it("refuses an opener whose location throws (cross-origin access)", () => {
    sessionStorage.setItem(OAUTH_POPUP_PENDING_KEY, "1");
    const postMessage = vi.fn();
    const opener = {
      get location(): { origin: string } {
        throw new Error("blocked cross-origin access");
      },
      postMessage,
      sessionStorage,
    };
    setOpener(opener);
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
    expect(postMessage).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it("refuses when the opener's sessionStorage cannot be read", () => {
    const postMessage = vi.fn();
    const opener = {
      location: { origin: window.location.origin },
      postMessage,
      get sessionStorage(): { getItem(key: string): string | null } {
        throw new Error("storage denied");
      },
    };
    setOpener(opener);
    const close = vi.spyOn(window, "close").mockImplementation(() => {});

    expect(notifyOAuthPopupOpener("google", "success")).toBe(false);
    expect(postMessage).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
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

describe("oauthPopupName", () => {
  it("keeps the eq-oauth prefix and never reuses a name", () => {
    const first = oauthPopupName("google");
    const second = oauthPopupName("google");
    expect(first.startsWith(`${OAUTH_POPUP_NAME_PREFIX}google-`)).toBe(true);
    expect(second.startsWith(`${OAUTH_POPUP_NAME_PREFIX}google-`)).toBe(true);
    expect(first).not.toBe(second);
  });
});

describe("pending marker refcount", () => {
  it("keeps the marker while another attempt is still pending", () => {
    expect(markOAuthPopupPending()).toBe(true);
    expect(markOAuthPopupPending()).toBe(true);
    expect(sessionStorage.getItem(OAUTH_POPUP_PENDING_KEY)).toBe("1");

    clearOAuthPopupPending();
    expect(sessionStorage.getItem(OAUTH_POPUP_PENDING_KEY)).toBe("1");

    clearOAuthPopupPending();
    expect(sessionStorage.getItem(OAUTH_POPUP_PENDING_KEY)).toBeNull();
  });

  it("returns false when the marker cannot be written", () => {
    const setItem = vi.spyOn(sessionStorage, "setItem").mockImplementation(() => {
      throw new Error("storage blocked");
    });
    expect(markOAuthPopupPending()).toBe(false);
    setItem.mockRestore();
  });
});
