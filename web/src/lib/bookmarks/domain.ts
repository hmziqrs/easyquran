import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import {
  SyncPausedError,
  type SyncDomain,
  type SyncMutation,
  type SyncRoundResult,
} from "#lib/sync/index.js";

import { decodeBookmarksEnvelope, type BookmarksMutation, type BookmarksSnapshot } from "./schema";

export interface BookmarkAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

export interface BookmarkAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

const SYNC_PATH = "/bookmark/v1/sync";

export const BOOKMARKS_DOMAIN = "bookmarks";

export function bookmarksDomainFor(userId: number): string {
  return `${BOOKMARKS_DOMAIN}.user:${userId}`;
}

export function createBookmarksDomain(
  auth: BookmarkAuthLike,
  state: BookmarkAuthStateLike,
  ownerId: number,
  isCurrent: () => boolean = () => true,
  onAccountMismatch?: () => void,
): SyncDomain<BookmarksMutation, BookmarksSnapshot> {
  const name = bookmarksDomainFor(ownerId);
  const assertCurrent = (signal?: AbortSignal): void => {
    if (signal?.aborted || !isCurrent() || !state.authenticated || state.user?.id !== ownerId) {
      throw new SyncPausedError("bookmarks sync paused: account changed");
    }
  };
  return {
    name,
    async sync(
      mutations: SyncMutation<BookmarksMutation>[],
      signal?: AbortSignal,
    ): Promise<SyncRoundResult<BookmarksSnapshot>> {
      assertCurrent(signal);
      if (mutations.some((mutation) => mutation.domain !== name)) {
        throw new SyncPausedError("bookmarks sync paused: mutation owner mismatch");
      }
      const res = await auth.unsafeRequest<unknown>(SYNC_PATH, {
        method: "POST",
        body: {
          expectedUserId: ownerId,
          mutations: mutations.map((mutation) => mutation.payload),
        },
        signal,
      });
      assertCurrent(signal);
      if (!res.ok) {
        if (res.status === 403 && res.error?.message === "Bookmark sync account changed") {
          onAccountMismatch?.();
          throw new SyncPausedError("bookmarks sync paused: cookie account changed");
        }
        throw new Error(`bookmark sync failed (${res.status})`);
      }
      const envelope = decodeBookmarksEnvelope(res.data);
      if (envelope === null) throw new Error("bookmark sync returned an unreadable snapshot");
      return {
        applied: envelope.applied,
        skipped: envelope.skipped,
        state: { folders: envelope.folders, bookmarks: envelope.bookmarks },
      };
    },
    applyServer(): void {},
  };
}
