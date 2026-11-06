import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { SyncPausedError, type RowRoundResult, type SyncDomain } from "#lib/sync/index.js";
import { SYNC_CLIENT, throwForFailure } from "#lib/sync/round-status.js";

import { decodeBookmarksRound, type BookmarksRound, type BookmarksWireRow } from "./schema";

export interface BookmarkAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

export interface BookmarkAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

/** What the store gives each round. Its dirty rows are the queue. */
export interface BookmarksRoundPort {
  dirtyRows(): readonly BookmarksWireRow[];
  position(): { readonly since: number; readonly after: number };
  applyRound(round: BookmarksRound, sent: readonly BookmarksWireRow[]): { readonly more: boolean };
  onUpgradeRequired?(): void;
}

const SYNC_PATH = "/bookmark/v1/sync";

export const BOOKMARKS_DOMAIN = "bookmarks";

export function bookmarksDomainFor(userId: number): string {
  return `${BOOKMARKS_DOMAIN}.user:${userId}`;
}

/**
 * Per-owner bookmark domain: sends the store's dirty rows to
 * `POST /bookmark/v1/sync` and hands the pulled page back.
 */
export function createBookmarksDomain(
  auth: BookmarkAuthLike,
  state: BookmarkAuthStateLike,
  ownerId: number,
  port: BookmarksRoundPort,
  isCurrent: () => boolean = () => true,
  onAccountMismatch?: () => void,
): SyncDomain<never, never> {
  const assertCurrent = (signal?: AbortSignal): void => {
    if (signal?.aborted || !isCurrent() || !state.authenticated || state.user?.id !== ownerId) {
      throw new SyncPausedError("bookmarks sync paused: account changed");
    }
  };
  return {
    name: bookmarksDomainFor(ownerId),
    async sync(): Promise<never> {
      throw new Error("bookmarks sync through rounds, not the outbox");
    },
    applyServer(): void {},
    async round(signal?: AbortSignal): Promise<RowRoundResult> {
      assertCurrent(signal);
      const sent = port.dirtyRows();
      const res = await auth.unsafeRequest<unknown>(SYNC_PATH, {
        method: "POST",
        body: {
          expectedUserId: ownerId,
          client: SYNC_CLIENT,
          ...port.position(),
          mutations: sent,
        },
        signal,
      });
      assertCurrent(signal);
      if (!res.ok) {
        throwForFailure("bookmarks sync", res, {
          mismatchMessage: "Bookmark sync account changed",
          onAccountMismatch,
          onUpgradeRequired: () => port.onUpgradeRequired?.(),
        });
      }
      const round = decodeBookmarksRound(res.data);
      if (round === null) throw new Error("bookmarks sync returned an unreadable round");
      return port.applyRound(round, sent);
    },
  };
}
