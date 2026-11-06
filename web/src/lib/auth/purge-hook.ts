import type { AuthTransitionContext, AuthTransitionHook } from "#lib/auth/auth-state.svelte.js";
import { bookmarks } from "#lib/bookmarks/store.svelte.js";
import { scheduleDeviceRegistration } from "#lib/device/register.js";
import { purgeUserCaches } from "#lib/offline/messages.js";
import { preferences } from "#lib/preferences/store.svelte.js";
import { sessions } from "#lib/sessions/store.svelte.js";

const ACCOUNT_TRANSITION_KINDS: ReadonlySet<AuthTransitionContext["kind"]> = new Set([
  "login",
  "oauth",
  "passkey",
  "logout",
  "current-session-terminated",
]);

/**
 * §9.5 policy: device-local reader data (position, recents, progress, notes)
 * survives logout AND expiry — only the account view is hidden. The old
 * `clearReadingPosition()` wipe on explicit logout (but not expiry) was
 * inconsistent and destroyed offline reading state; it stays out (EC-14/16).
 * Session sync follows the same edges: register on login, quarantine (never
 * destroy) the account's outbox partition on logout/expiry.
 */
export function makePurgeHook(): (ctx: AuthTransitionContext) => Promise<void> {
  return async (ctx: AuthTransitionContext): Promise<void> => {
    if (ACCOUNT_TRANSITION_KINDS.has(ctx.kind)) {
      bookmarks.suspendAuth();
      sessions.onAuthTransition();
      preferences.onAuthTransition();
    }
    await purgeUserCaches();
  };
}

const SIGN_IN_KINDS: ReadonlySet<AuthTransitionContext["kind"]> = new Set([
  "login",
  "oauth",
  "passkey",
]);

export function installPurgeHook(state: {
  setOnAuthTransition: (hook: AuthTransitionHook) => void;
  readonly user: { readonly id: number } | null;
}): void {
  const purge = makePurgeHook();
  state.setOnAuthTransition(async (ctx: AuthTransitionContext): Promise<void> => {
    await purge(ctx);
    // Device registration waits for the new user to land on the auth state.
    if (SIGN_IN_KINDS.has(ctx.kind)) scheduleDeviceRegistration(() => state.user?.id ?? null);
  });
}
