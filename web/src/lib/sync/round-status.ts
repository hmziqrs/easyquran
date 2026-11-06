import type { AuthRequestResult } from "#lib/auth/auth-client.js";

import { SyncPausedError, SyncRetryAfterError, SyncSnapshotTooLargeError } from "./types";

/** The wire protocol this build speaks (contracts/sync.md). */
export const SYNC_PROTOCOL = 1;
export const SYNC_CLIENT = { protocol: SYNC_PROTOCOL, platform: "web" };
const DEFAULT_RETRY_AFTER_S = 60;

export interface FailureHandlers {
  /** The server's message when the cookie's account is not the expected one. */
  readonly mismatchMessage: string;
  readonly onAccountMismatch?: () => void;
  readonly onUpgradeRequired?: () => void;
}

/** The status table in contracts/sync.md, the same on every client. */
export function throwForFailure(
  label: string,
  res: AuthRequestResult<unknown>,
  handlers: FailureHandlers,
): never {
  const { status } = res;
  if (status === 403 && res.error?.message === handlers.mismatchMessage) {
    handlers.onAccountMismatch?.();
  }
  if (status === 401 || status === 403) throw new SyncPausedError(`${label} paused (${status})`);
  if (status === 426) {
    handlers.onUpgradeRequired?.();
    throw new SyncPausedError(`${label} paused: update required`);
  }
  if (status === 429) throw new SyncRetryAfterError(res.retryAfter ?? DEFAULT_RETRY_AFTER_S);
  if (status === 507) throw new SyncSnapshotTooLargeError(label);
  // 404 (auth nests unmounted), 5xx, and a malformed-request 400: back off, rows stay dirty.
  throw new Error(`${label} failed (${status})`);
}

/**
 * `dirty` after a merge. It is OR across tabs, except that an ack is final for the
 * stamps it named: identical stamps clean on either side are clean, and a dirty side
 * whose registers all lost has nothing left to send.
 */
export function mergedDirty(
  a: { readonly dirty: boolean; readonly stamps: readonly (string | null)[] },
  b: { readonly dirty: boolean; readonly stamps: readonly (string | null)[] },
  merged: readonly (string | null)[],
): boolean {
  const same = a.stamps.every((stamp, i) => stamp === b.stamps[i]);
  if (same) return a.dirty && b.dirty;
  const contributes = (x: typeof a, y: typeof a): boolean =>
    merged.some((stamp, i) => x.stamps[i] === stamp && y.stamps[i] !== stamp);
  return (a.dirty && contributes(a, b)) || (b.dirty && contributes(b, a));
}
