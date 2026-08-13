import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$lib/offline/messages", () => ({
  purgeUserCaches: vi.fn().mockResolvedValue(undefined),
}));

import { installPurgeHook, makePurgeHook } from "$lib/auth/purge-hook";
import { purgeUserCaches } from "$lib/offline/messages";
import { reader } from "$lib/stores/reader.svelte";

function fakeState(): { setOnAuthTransition: ReturnType<typeof vi.fn> } {
  return { setOnAuthTransition: vi.fn() };
}

describe("installPurgeHook", () => {
  it("registers a transition hook that calls purgeUserCaches", () => {
    const state = fakeState();
    installPurgeHook(state as never);
    expect(state.setOnAuthTransition).toHaveBeenCalledTimes(1);
    const hook = state.setOnAuthTransition.mock.calls[0]![0]! as (ctx: unknown) => Promise<void>;
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

describe("makePurgeHook reading-position clear", () => {
  it("clears the reader's reading position on logout", async () => {
    reader.openVerse(2, 255, "uthmani");
    expect(reader.hasLastRead).toBe(true);
    const hook = makePurgeHook();
    await hook({ kind: "logout" });
    expect(reader.hasLastRead).toBe(false);
  });

  it("leaves the reader's reading position intact on login", async () => {
    reader.openVerse(2, 255, "uthmani");
    const hook = makePurgeHook();
    await hook({ kind: "login" });
    expect(reader.hasLastRead).toBe(true);
    reader.clearReadingPosition();
  });
});
