import type { AuthTransitionContext, AuthTransitionHook } from "$lib/auth/auth-state.svelte";
import { purgeUserCaches } from "$lib/offline/messages";
import { reader } from "$lib/stores/reader.svelte";

const CLEAR_POSITION_KINDS: ReadonlySet<AuthTransitionContext["kind"]> = new Set([
  "logout",
  "current-session-terminated",
]);

export function makePurgeHook(): (ctx: AuthTransitionContext) => Promise<void> {
  return async (ctx: AuthTransitionContext): Promise<void> => {
    if (CLEAR_POSITION_KINDS.has(ctx.kind)) reader.clearReadingPosition();
    await purgeUserCaches();
  };
}

export function installPurgeHook(state: {
  setOnAuthTransition: (hook: AuthTransitionHook) => void;
}): void {
  state.setOnAuthTransition(makePurgeHook());
}
