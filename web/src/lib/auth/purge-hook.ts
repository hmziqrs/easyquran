import type { AuthTransitionContext, AuthTransitionHook } from "#lib/auth/auth-state.svelte.js";
import { bookmarks } from "#lib/bookmarks/store.svelte.js";
import { purgeUserCaches } from "#lib/offline/messages.js";
import { reader } from "#lib/stores/reader.svelte.js";

const CLEAR_POSITION_KINDS: ReadonlySet<AuthTransitionContext["kind"]> = new Set([
  "logout",
  "current-session-terminated",
]);

const ACCOUNT_TRANSITION_KINDS: ReadonlySet<AuthTransitionContext["kind"]> = new Set([
  "login",
  "oauth",
  "passkey",
  "logout",
  "current-session-terminated",
]);

export function makePurgeHook(): (ctx: AuthTransitionContext) => Promise<void> {
  return async (ctx: AuthTransitionContext): Promise<void> => {
    if (ACCOUNT_TRANSITION_KINDS.has(ctx.kind)) bookmarks.suspendAuth();
    if (CLEAR_POSITION_KINDS.has(ctx.kind)) reader.clearReadingPosition();
    await purgeUserCaches();
  };
}

export function installPurgeHook(state: {
  setOnAuthTransition: (hook: AuthTransitionHook) => void;
}): void {
  state.setOnAuthTransition(makePurgeHook());
}
