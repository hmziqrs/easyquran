import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import {
  SyncPausedError,
  SyncRetryAfterError,
  type RowRoundResult,
  type SyncDomain,
} from "#lib/sync/index.js";

import { decodeSessionRound, type SessionRound, type SessionWireRow } from "./schema";

export interface SessionsAuthLike {
  unsafeRequest<T>(path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>>;
}

export interface SessionsAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

/** What the store gives each round. Its dirty rows are the queue. */
export interface SessionsRoundPort {
  /** Dirty rows to send, oldest first. */
  dirtyRows(): readonly SessionWireRow[];
  /** Where the pull stands. */
  position(): { readonly since: number; readonly after: number };
  /** A durable apply, so `since` only moves after the page landed. */
  applyRound(round: SessionRound, sent: readonly SessionWireRow[]): { readonly more: boolean };
  /** 426: pause sync, show an update prompt, keep reading. */
  onUpgradeRequired?(): void;
}

const SYNC_PATH = "/reading-session/v1/sync";
/** The wire protocol this build speaks (contracts/sync.md). */
export const SYNC_PROTOCOL = 1;
const CLIENT = { protocol: SYNC_PROTOCOL, platform: "web" };
const DEFAULT_RETRY_AFTER_S = 60;

export const SESSIONS_DOMAIN = "sessions";

export function sessionsDomainFor(userId: number): string {
  return `${SESSIONS_DOMAIN}.user:${userId}`;
}

/** The status table in contracts/sync.md, the same on every client. */
function classifyFailure(
  res: AuthRequestResult<unknown>,
  onAccountMismatch: (() => void) | undefined,
  onUpgradeRequired: (() => void) | undefined,
): never {
  const { status } = res;
  if (status === 403 && res.error?.message === "Reading session sync account changed") {
    onAccountMismatch?.();
  }
  if (status === 401 || status === 403) {
    throw new SyncPausedError(`reading-session sync paused (${status})`);
  }
  if (status === 426) {
    onUpgradeRequired?.();
    throw new SyncPausedError("reading-session sync paused: update required");
  }
  if (status === 429) throw new SyncRetryAfterError(res.retryAfter ?? DEFAULT_RETRY_AFTER_S);
  // 404 (auth nests unmounted), 5xx, and a malformed-request 400: back off, rows stay dirty.
  throw new Error(`reading-session sync failed (${status})`);
}

/**
 * Per-owner reading-thread domain: sends the store's dirty rows to
 * `POST /reading-session/v1/sync` and hands the pulled page back.
 */
export function createSessionsDomain(
  auth: SessionsAuthLike,
  state: SessionsAuthStateLike,
  ownerId: number,
  port: SessionsRoundPort,
  isCurrent: () => boolean = () => true,
  onAccountMismatch?: () => void,
): SyncDomain<never, never> {
  const assertCurrent = (signal?: AbortSignal): void => {
    if (signal?.aborted || !isCurrent() || !state.authenticated || state.user?.id !== ownerId) {
      throw new SyncPausedError("reading-session sync paused: account changed");
    }
  };
  return {
    name: sessionsDomainFor(ownerId),
    async sync(): Promise<never> {
      throw new Error("reading sessions sync through rounds, not the outbox");
    },
    applyServer(): void {},
    async round(signal?: AbortSignal): Promise<RowRoundResult> {
      assertCurrent(signal);
      const sent = port.dirtyRows();
      const res = await auth.unsafeRequest<unknown>(SYNC_PATH, {
        method: "POST",
        body: {
          expectedUserId: ownerId,
          client: CLIENT,
          ...port.position(),
          mutations: sent,
        },
        signal,
      });
      assertCurrent(signal);
      if (!res.ok) classifyFailure(res, onAccountMismatch, () => port.onUpgradeRequired?.());
      const round = decodeSessionRound(res.data);
      if (round === null) throw new Error("reading-session sync returned an unreadable round");
      return port.applyRound(round, sent);
    },
  };
}
