import { authState } from "#lib/auth/auth-state.svelte.js";
import { consent } from "#lib/stores/consent.svelte.js";

/** Keeps the consent gate in step with the signed-in account's minor flag (A34). */
export function startMinorGate(): () => void {
  return $effect.root(() => {
    $effect(() => {
      consent.setMinorAccount(authState.user?.is_minor === true);
    });
  });
}
