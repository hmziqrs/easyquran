import type { AuthRequestResult, UnsafeRequestInit } from "$lib/auth/auth-client";
import type { SyncDomain, SyncMutation, SyncRoundResult } from "$lib/sync";
import { decodeBookmarksEnvelope, type BookmarksMutation, type BookmarksSnapshot } from "./schema";

/**
 * Transport seam over AuthClient.unsafeRequest (session cookie + CSRF handled
 * there). Tests inject a fake with the same single method.
 */
export interface BookmarkAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

const SYNC_PATH = "/bookmark/v1/sync";

export const BOOKMARKS_DOMAIN = "bookmarks";

/**
 * The bookmarks sync domain: pushes queued mutations (payloads verbatim, FIFO
 * order preserved by the engine) and returns the decoded server snapshot as the
 * round state. Any non-2xx throws so the engine keeps the queue and backs off;
 * the same applies to an unreadable body (treated as a failed round, not as an
 * empty snapshot — an empty snapshot would erase the local view).
 */
export function createBookmarksDomain(
  auth: BookmarkAuthLike,
): SyncDomain<BookmarksMutation, BookmarksSnapshot> {
  return {
    name: BOOKMARKS_DOMAIN,
    async sync(
      mutations: SyncMutation<BookmarksMutation>[],
    ): Promise<SyncRoundResult<BookmarksSnapshot>> {
      const res = await auth.unsafeRequest<unknown>(SYNC_PATH, {
        method: "POST",
        body: { mutations: mutations.map((mutation) => mutation.payload) },
      });
      if (!res.ok) throw new Error(`bookmark sync failed (${res.status})`);
      const envelope = decodeBookmarksEnvelope(res.data);
      if (envelope === null) throw new Error("bookmark sync returned an unreadable snapshot");
      return {
        applied: envelope.applied,
        skipped: envelope.skipped,
        state: { folders: envelope.folders, bookmarks: envelope.bookmarks },
      };
    },
    applyServer(): void {
      // The store registers an adapter that routes snapshots into its $state;
      // the bare domain has no local view of its own.
    },
  };
}
