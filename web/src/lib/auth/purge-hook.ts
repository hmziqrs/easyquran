import { purgeUserCaches } from "$lib/offline/messages";
import type { AuthTransitionContext, AuthTransitionHook } from "$lib/auth/auth-state.svelte";

export function makePurgeHook(): (ctx: AuthTransitionContext) => Promise<void> {
  return (_ctx: AuthTransitionContext): Promise<void> => purgeUserCaches();
}

export function installPurgeHook(state: {
  setOnAuthTransition: (hook: AuthTransitionHook) => void;
}): void {
  state.setOnAuthTransition(makePurgeHook());
}
