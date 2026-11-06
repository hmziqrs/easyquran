import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { createSyncEngine } from "#lib/sync/engine.svelte.js";
import { createOutbox, memoryQueueStorage } from "#lib/sync/outbox.js";
import {
  SyncRetryAfterError,
  SyncSnapshotTooLargeError,
  type RowRoundResult,
  type SyncDomain,
  type SyncRoundOptions,
} from "#lib/sync/types.js";

vi.mock("$app/env", () => ({ browser: true }));

interface RoundCall {
  readonly pullOnly: boolean;
}

interface DomainState {
  pushHeld: boolean;
  failWith: Error | null;
}

/** A row domain whose pushes can be held by a 429 while its pulls stay open. */
function roundDomain(options: { pushHeld: boolean }) {
  const calls: RoundCall[] = [];
  const state: DomainState = { pushHeld: options.pushHeld, failWith: null };
  const domain: SyncDomain<never, never> = {
    name: "bookmarks.user:1",
    async sync(): Promise<never> {
      throw new Error("rounds only");
    },
    applyServer(): void {},
    async round(_signal?: AbortSignal, round?: SyncRoundOptions): Promise<RowRoundResult> {
      const pullOnly = round?.pullOnly === true;
      calls.push({ pullOnly });
      if (state.failWith !== null) throw state.failWith;
      if (!pullOnly && state.pushHeld) throw new SyncRetryAfterError(60);
      return { more: false };
    },
  };
  return { domain, calls, state };
}

function engineFor(domain: SyncDomain<never, never>) {
  return createSyncEngine({
    outbox: createOutbox(memoryQueueStorage()),
    domains: [domain],
    online: () => true,
    flushDelayMs: 2_000,
  });
}

afterEach(() => {
  vi.useRealTimers();
});

describe("a held push never holds the pull", () => {
  it("a 429 on pushes holds the pushes and still pulls in the same flush", async () => {
    const { domain, calls } = roundDomain({ pushHeld: true });
    const engine = engineFor(domain);
    await engine.hydrate();

    await engine.flush();

    expect(calls).toEqual([{ pullOnly: false }, { pullOnly: true }]);
    expect(engine.lastError).toBeNull();
  });

  it("during the Retry-After hold each flush pulls, and pushes resume once it lapses", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-06T10:00:00Z"));
    const { domain, calls, state } = roundDomain({ pushHeld: true });
    const engine = engineFor(domain);
    await engine.hydrate();

    await engine.flush();
    await engine.flush();
    // The held flush pulls and never pushes.
    expect(calls.slice(2)).toEqual([{ pullOnly: true }]);

    state.pushHeld = false;
    vi.setSystemTime(new Date("2026-11-06T10:01:01Z"));
    await engine.flush();
    expect(calls.at(-1)).toEqual({ pullOnly: false });
  });

  it("a snapshot over the quota (507) is a surfaced failure, named as such", async () => {
    const { domain, state } = roundDomain({ pushHeld: false });
    state.failWith = new SyncSnapshotTooLargeError("bookmarks sync");
    const engine = engineFor(domain);
    await engine.hydrate();

    await engine.flush();

    expect(engine.phase).toBe("error");
    expect(engine.lastError).toContain("507");
    expect(engine.lastError).toContain("bookmarks sync");
  });
});
