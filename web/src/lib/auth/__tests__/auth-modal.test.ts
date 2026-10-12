import { afterEach, describe, expect, it } from "vite-plus/test";

import { authModal } from "#lib/auth/auth-modal.svelte.js";
import { getReturnTarget, setReturnTarget } from "#lib/auth/return-target.js";

afterEach(() => {
  sessionStorage.clear();
  history.replaceState(null, "", "/");
  authModal.close();
});

describe("authModal.show return target", () => {
  it("an explicit target is stored for the post-auth redirect", () => {
    authModal.show("login", "/bookmarks?filter=all");
    expect(authModal.open).toBe(true);
    expect(getReturnTarget()).toBe("/bookmarks?filter=all");
  });

  it("without a target, the current in-app page is captured", () => {
    history.replaceState(null, "", "/surah/2?t=saheeh");
    authModal.show("register");
    expect(getReturnTarget()).toBe("/surah/2?t=saheeh");
    expect(authModal.mode).toBe("register");
  });

  it("opening from an auth surface never clobbers a stored target", () => {
    setReturnTarget("/bookmarks");
    history.replaceState(null, "", "/login?returnTo=/bookmarks");
    authModal.show("login");
    expect(getReturnTarget()).toBe("/bookmarks");
  });

  it("an invalid explicit target is rejected and nothing is stored", () => {
    authModal.show("login", "https://evil.example");
    expect(getReturnTarget()).toBeNull();
  });
});
