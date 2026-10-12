import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("#lib/offline/messages.js", () => ({
  purgeUserCaches: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: "local",
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import type { AuthTransitionContext } from "#lib/auth/auth-state.svelte.js";
import { installPurgeHook, makePurgeHook } from "#lib/auth/purge-hook.js";
import { purgeUserCaches } from "#lib/offline/messages.js";
import { reader } from "#lib/stores/reader.svelte.js";

function fakeState() {
  return { setOnAuthTransition: vi.fn() };
}

describe("installPurgeHook", () => {
  it("registers a transition hook that calls purgeUserCaches", () => {
    const state = fakeState();
    // SAFETY: installPurgeHook reads only setOnAuthTransition, which this double provides as a vi.fn().
    installPurgeHook(state as never);
    expect(state.setOnAuthTransition).toHaveBeenCalledTimes(1);
    // SAFETY: the first argument of the first setOnAuthTransition call is makePurgeHook()'s hook, typed (ctx: AuthTransitionContext) => Promise<void>.
    const hook = state.setOnAuthTransition.mock.calls[0]![0]! as (
      ctx: AuthTransitionContext,
    ) => Promise<void>;
    void hook({ kind: "login" });
    expect(purgeUserCaches).toHaveBeenCalled();
  });

  it("makePurgeHook resolves after purgeUserCaches resolves", async () => {
    const hook = makePurgeHook();
    await expect(hook({ kind: "logout" })).resolves.toBeUndefined();
    expect(purgeUserCaches).toHaveBeenCalled();
  });

  it("reinstalling overwrites the previous hook (idempotent registration)", () => {
    const state = fakeState();
    // SAFETY: installPurgeHook reads only setOnAuthTransition, which this double provides as a vi.fn().
    installPurgeHook(state as never);
    // SAFETY: installPurgeHook reads only setOnAuthTransition, which this double provides as a vi.fn().
    installPurgeHook(state as never);
    expect(state.setOnAuthTransition).toHaveBeenCalledTimes(2);
  });
});

describe("makePurgeHook reading-position policy (§9.5)", () => {
  it("keeps the reader's reading position on logout — device-local data survives (EC-14)", async () => {
    reader.openVerse(2, 255, "uthmani");
    expect(reader.hasLastRead).toBe(true);
    const hook = makePurgeHook();
    await hook({ kind: "logout" });
    expect(reader.hasLastRead).toBe(true);
  });

  it("keeps the reading position on session expiry/terminate too (EC-16)", async () => {
    reader.openVerse(2, 255, "uthmani");
    const hook = makePurgeHook();
    await hook({ kind: "current-session-terminated" });
    expect(reader.hasLastRead).toBe(true);
  });

  it("leaves the reader's reading position intact on login", async () => {
    reader.openVerse(2, 255, "uthmani");
    const hook = makePurgeHook();
    await hook({ kind: "login" });
    expect(reader.hasLastRead).toBe(true);
    reader.clearReadingPosition();
  });
});
