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
import { sessions } from "#lib/sessions/store.svelte.js";

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

describe("makePurgeHook reading-thread policy", () => {
  it("keeps device reading threads across logout, expiry and login (never destroyed)", async () => {
    const first = sessions.enter({
      scope: "surah:2",
      surah: 2,
      ayah: 1,
      exact: false,
      sourceId: null,
    });
    sessions.cursor(2, 255, null);
    expect(sessions.current?.id).toBe(first.thread.id);
    for (const kind of ["logout", "current-session-terminated", "login"] as const) {
      await makePurgeHook()({ kind });
      expect(sessions.open.some((thread) => thread.id === first.thread.id)).toBe(true);
    }
  });
});
