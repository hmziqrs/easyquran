import { bookmarksDomainFor } from "#lib/bookmarks/domain.js";
import { sessionsDomainFor } from "#lib/sessions/domain.js";
import { syncEngine } from "#lib/sync/index.js";

/**
 * §9.5 (EC-14, the "notice still to build" half): when an explicit sign-out
 * leaves unsynced mutations in the account's outbox partitions, the sign-out
 * surface shows a one-line notice — "Your unsynced changes are kept on this
 * device for your next sign-in". This module is the state behind that copy:
 * armed by LogoutFlow before the view tears down (the counts must be read
 * while the owner is still current — after `suspendAuth()` the stores report
 * 0), consumed once by whatever renders the notice after the redirect, and
 * cleared on the next successful sign-in so it can never go stale.
 */
let armed = $state(false);

/** Reactive read for renderers; does not consume. */
export function signOutNoticeArmed(): boolean {
  return armed;
}

/** One-shot read for the post-redirect surface. */
export function consumeSignOutNotice(): boolean {
  const wasArmed = armed;
  armed = false;
  return wasArmed;
}

/** Any authenticated edge (sign-in, cross-tab adoption) invalidates a stale notice. */
export function clearSignOutNotice(): void {
  armed = false;
}

/**
 * Arm the notice iff the given account still has queued work in either sync
 * partition. Injection seam for tests; the default reads the engine's durable
 * pending counters (partition names carry the owner, so a different account's
 * quarantine is never counted).
 */
export function armSignOutNotice(
  userId: number | null,
  pendingFor: (domain: string) => number = (domain) => syncEngine.pendingFor(domain),
): void {
  if (userId === null) {
    armed = false;
    return;
  }
  armed = pendingFor(bookmarksDomainFor(userId)) > 0 || pendingFor(sessionsDomainFor(userId)) > 0;
}
