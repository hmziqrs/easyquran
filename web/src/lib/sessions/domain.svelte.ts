import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import {
  SyncPausedError,
  SyncRejectedError,
  type SyncDomain,
  type SyncMutation,
  type SyncRoundResult,
} from "#lib/sync/index.js";

import { decodeSessionSnapshot, type SessionMutation, type SessionSnapshot } from "./schema.svelte";

export interface SessionsAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

export interface SessionsAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

const SYNC_PATH = "/reading-session/v1/sync";

export const SESSIONS_DOMAIN = "sessions";

export function sessionsDomainFor(userId: number): string {
  return `${SESSIONS_DOMAIN}.user:${userId}`;
}

/**
 * Per-owner reading-session domain (§9.2): pushes queued mutations to
 * `POST /reading-session/v1/sync` and returns the account's full snapshot.
 * `expectedUserId` is required — no legacy clients exist for this module.
 */
export function createSessionsDomain(
  auth: SessionsAuthLike,
  state: SessionsAuthStateLike,
  ownerId: number,
  isCurrent: () => boolean = () => true,
  onAccountMismatch?: () => void,
): SyncDomain<SessionMutation, SessionSnapshot> {
  const name = sessionsDomainFor(ownerId);
  const assertCurrent = (signal?: AbortSignal): void => {
    if (signal?.aborted || !isCurrent() || !state.authenticated || state.user?.id !== ownerId) {
      throw new SyncPausedError("reading-session sync paused: account changed");
    }
  };
  return {
    name,
    async sync(
      mutations: SyncMutation<SessionMutation>[],
      signal?: AbortSignal,
    ): Promise<SyncRoundResult<SessionSnapshot>> {
      assertCurrent(signal);
      if (mutations.some((mutation) => mutation.domain !== name)) {
        throw new SyncPausedError("reading-session sync paused: mutation owner mismatch");
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
        if (res.status === 403 && res.error?.message === "Reading session sync account changed") {
          onAccountMismatch?.();
          throw new SyncPausedError("reading-session sync paused: cookie account changed");
        }
        // Same ledger split as bookmarks (EC-29): a 400 is the server's
        // durable validation verdict on the batch; 401/429/5xx stay generic.
        if (res.status === 400) {
          throw new SyncRejectedError(`reading-session sync rejected (${res.status})`);
        }
        throw new Error(`reading-session sync failed (${res.status})`);
      }
      const snapshot = decodeSessionSnapshot(res.data);
      if (snapshot === null)
        throw new Error("reading-session sync returned an unreadable snapshot");
      return { applied: snapshot.applied, skipped: snapshot.skipped, state: snapshot };
    },
    applyServer(): void {},
  };
}
