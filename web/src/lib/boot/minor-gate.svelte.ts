import { authState } from "#lib/auth/auth-state.svelte.js";
import { consent } from "#lib/stores/consent.svelte.js";

/**
 * Keeps the consent gate in step with the signed-in account (A34). The minor flag is
 * applied first, then the gate opens once auth status is known. Until then analytics
 * and crash reporting are held, so a minor's first page view cannot leave before the
 * flag is read.
 */
export function startMinorGate(): () => void {
  return $effect.root(() => {
    $effect(() => {
      consent.setMinorAccount(authState.user?.is_minor === true);
      if (authState.status !== "unknown") consent.markAuthResolved();
    });
  });
}
