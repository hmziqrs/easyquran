import type { AuthRequestResult, UnsafeRequestInit } from "$lib/auth/auth-client";
import { SyncPausedError, type SyncMutation } from "$lib/sync";
import { afterEach, describe, expect, it } from "vite-plus/test";

import { createBookmarksDomain, type BookmarkAuthLike } from "../domain";
import type { BookmarksMutation } from "../schema";

interface FakeAuth {
  readonly auth: BookmarkAuthLike;
  /** One entry per unsafeRequest call: [path, init]. */
  readonly calls: Array<[string, UnsafeRequestInit]>;
  respondWith(result: AuthRequestResult<unknown>): void;
}

function fakeAuth(): FakeAuth {
  const calls: Array<[string, UnsafeRequestInit]> = [];
  let next: AuthRequestResult<unknown> = {
    ok: true,
    status: 200,
    data: null,
    error: null,
    rotated: false,
  };
  const handle: FakeAuth = {
    calls,
    respondWith(result) {
      next = result;
    },
    auth: {
      async unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>> {
        calls.push([path, init]);
        // SAFETY: test double — `next` is a fixture result only these tests set; T is the caller's claim over fixture data, never runtime-parsed input.
        return next as AuthRequestResult<T>;
      },
    },
  };
  return handle;
}

function mutation(payload: BookmarksMutation, seq: number): SyncMutation<BookmarksMutation> {
  return { id: `m${seq}`, domain: "bookmarks", seq, payload, queuedAt: 0 };
}

function okBody() {
  return {
    applied: 2,
    skipped: 1,
    folders: [
      { id: "f1", name: "Tafsir", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-02T00:00:00Z" },
    ],
    bookmarks: [
      {
        id: "b1",
        folderId: null,
        surah: 1,
        ayah: 1,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-02T00:00:00Z",
      },
    ],
  };
}

afterEach(() => {
  // Each test builds its own fake; nothing global to reset.
});

describe("createBookmarksDomain.sync", () => {
  it("throws SyncPausedError before any request while signed out", async () => {
    const fake = fakeAuth();
    const domain = createBookmarksDomain(fake.auth, { authenticated: false });

    const batch = [mutation({ kind: "bookmark.delete", id: "b1", updatedAt: "2026-01-01T00:00:00Z" }, 1)];
    await expect(domain.sync(batch)).rejects.toBeInstanceOf(SyncPausedError);
    await expect(domain.sync(batch)).rejects.toThrow("bookmarks sync paused");
    expect(fake.calls).toHaveLength(0);
  });

  it("posts mutation payloads verbatim under `mutations` and returns the decoded round", async () => {
    const fake = fakeAuth();
    fake.respondWith({ ok: true, status: 200, data: okBody(), error: null, rotated: false });
    const domain = createBookmarksDomain(fake.auth, { authenticated: true });

    const batch = [
      mutation({ kind: "bookmark.upsert", id: "b1", folderId: null, surah: 1, ayah: 1, updatedAt: "2026-01-01T00:00:00Z" }, 1),
      mutation({ kind: "folder.delete", id: "f9", updatedAt: "2026-01-01T00:00:01Z" }, 2),
    ];
    const result = await domain.sync(batch);

    expect(fake.calls).toHaveLength(1);
    const [path, init] = fake.calls[0]!;
    expect(path).toBe("/bookmark/v1/sync");
    expect(init.method).toBe("POST");
    expect(init.body).toEqual({
      mutations: [
        { kind: "bookmark.upsert", id: "b1", folderId: null, surah: 1, ayah: 1, updatedAt: "2026-01-01T00:00:00Z" },
        { kind: "folder.delete", id: "f9", updatedAt: "2026-01-01T00:00:01Z" },
      ],
    });
    expect(result.applied).toBe(2);
    expect(result.skipped).toBe(1);
    expect(result.state.folders).toEqual([
      { id: "f1", name: "Tafsir", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-02T00:00:00Z" },
    ]);
    expect(result.state.bookmarks).toEqual([
      { id: "b1", folderId: null, surah: 1, ayah: 1, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-02T00:00:00Z" },
    ]);
  });

  it("sends an empty mutations array for a pure pull", async () => {
    const fake = fakeAuth();
    fake.respondWith({ ok: true, status: 200, data: okBody(), error: null, rotated: false });
    const domain = createBookmarksDomain(fake.auth, { authenticated: true });

    await domain.sync([]);
    expect(fake.calls[0]![1].body).toEqual({ mutations: [] });
  });

  it("throws on any non-2xx so the engine keeps the queue", async () => {
    const fake = fakeAuth();
    const domain = createBookmarksDomain(fake.auth, { authenticated: true });

    for (const status of [400, 401, 409, 500, 503]) {
      fake.respondWith({ ok: false, status, data: null, error: { type: "AUTH_ERROR" }, rotated: false });
      await expect(domain.sync([mutation({ kind: "bookmark.delete", id: "b1", updatedAt: "2026-01-01T00:00:00Z" }, 1)])).rejects.toThrow(
        `bookmark sync failed (${status})`,
      );
    }
  });

  it("throws on a 200 with an unreadable body (never an empty snapshot)", async () => {
    const fake = fakeAuth();
    const domain = createBookmarksDomain(fake.auth, { authenticated: true });

    fake.respondWith({ ok: true, status: 200, data: null, error: null, rotated: false });
    await expect(domain.sync([])).rejects.toThrow("unreadable snapshot");

    fake.respondWith({ ok: true, status: 200, data: { applied: 1 }, error: null, rotated: false });
    await expect(domain.sync([])).rejects.toThrow("unreadable snapshot");
  });
});
