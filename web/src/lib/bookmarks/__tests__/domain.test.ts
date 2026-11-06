import { describe, expect, it, vi } from "vite-plus/test";

import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { SyncRetryAfterError, SyncSnapshotTooLargeError } from "#lib/sync/index.js";

import { createBookmarksDomain, type BookmarksRoundPort } from "../domain";
import type { BookmarksWireRow } from "../schema";

const ROW: BookmarksWireRow = {
  kind: "bookmark.upsert",
  id: "b1b1b1b1-b1b1-7b1b-8b1b-b1b1b1b1b1b1",
  stamp: "1762000000000-0000-dev",
  folderId: "d",
  surah: 2,
  ayah: 255,
  endSurah: 2,
  endAyah: 255,
  sourceId: null,
  deleted: false,
  updatedAt: "2026-11-06T00:00:00.000Z",
};

/** A valid empty round as the server sends it. */
const ROUND_JSON = {
  serverTime: 0,
  syncEpoch: "1",
  results: [],
  folders: [],
  deletedFolders: [],
  bookmarks: [],
  defaultFolderId: "d",
  rev: 0,
  latestRev: 0,
  more: false,
  tombstoneFloor: 0,
};

function fakePort() {
  const applied: Array<{ sent: readonly BookmarksWireRow[] }> = [];
  const dirty = vi.fn((): readonly BookmarksWireRow[] => [ROW]);
  const port: BookmarksRoundPort = {
    dirtyRows: dirty,
    position: () => ({ since: 0, after: -1 }),
    applyRound: (_round, sent) => {
      applied.push({ sent });
      return { more: false };
    },
  };
  return { port, applied, dirty };
}

interface SentBody {
  readonly mutations: readonly unknown[];
}

function fakeAuth(respond: (body: SentBody) => Partial<AuthRequestResult<unknown>>) {
  const bodies: SentBody[] = [];
  return {
    bodies,
    unsafeRequest: async <T>(_path: string, init: UnsafeRequestInit) => {
      // SAFETY: the domain always sends an object body with a `mutations` array.
      const body = init.body as SentBody;
      bodies.push(body);
      const result: AuthRequestResult<unknown> = {
        ok: true,
        status: 200,
        data: ROUND_JSON,
        error: null,
        rotated: false,
        ...respond(body),
      };
      // SAFETY: test double; the domain decodes `data` itself and never trusts T.
      return result as AuthRequestResult<T>;
    },
  };
}

const account = { authenticated: true, user: { id: 1 } };

describe("bookmarks round", () => {
  it("a pull-only round sends no rows and never reads the dirty queue", async () => {
    const auth = fakeAuth(() => ({}));
    const { port, applied, dirty } = fakePort();
    const domain = createBookmarksDomain(auth, account, 1, port);

    await domain.round?.(undefined, { pullOnly: true });

    expect(auth.bodies).toEqual([expect.objectContaining({ mutations: [] })]);
    expect(dirty).not.toHaveBeenCalled();
    expect(applied).toEqual([{ sent: [] }]);
  });

  it("a push answered with 429 holds the push with its Retry-After", async () => {
    const auth = fakeAuth(() => ({ ok: false, status: 429, retryAfter: 120 }));
    const { port } = fakePort();
    const domain = createBookmarksDomain(auth, account, 1, port);

    await expect(domain.round?.()).rejects.toBeInstanceOf(SyncRetryAfterError);
    await expect(domain.round?.()).rejects.toMatchObject({ seconds: 120 });
  });

  it("507 surfaces as a snapshot failure, not a generic retry", async () => {
    const auth = fakeAuth(() => ({ ok: false, status: 507 }));
    const { port } = fakePort();
    const domain = createBookmarksDomain(auth, account, 1, port);

    await expect(domain.round?.(undefined, { pullOnly: true })).rejects.toBeInstanceOf(
      SyncSnapshotTooLargeError,
    );
  });
});
