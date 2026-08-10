import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/offline/messages", () => ({
  purgeUserCaches: vi.fn().mockResolvedValue(undefined),
}));

import { installPurgeHook, makePurgeHook } from "$lib/auth/purge-hook";
import { purgeUserCaches } from "$lib/offline/messages";

function fakeState(): { setOnAuthTransition: ReturnType<typeof vi.fn> } {
  return { setOnAuthTransition: vi.fn() };
}

describe("installPurgeHook", () => {
  it("registers a transition hook that calls purgeUserCaches", () => {
    const state = fakeState();
    installPurgeHook(state as never);
    expect(state.setOnAuthTransition).toHaveBeenCalledTimes(1);
    const hook = state.setOnAuthTransition.mock.calls[0][0] as (ctx: unknown) => Promise<void>;
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
    installPurgeHook(state as never);
    installPurgeHook(state as never);
    expect(state.setOnAuthTransition).toHaveBeenCalledTimes(2);
  });
});
