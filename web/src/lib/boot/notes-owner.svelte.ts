import { untrack } from "svelte";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { reader } from "#lib/stores/reader.svelte.js";

/**
 * Re-resolves the notes owner whenever auth changes (EC-17). Runs as an effect so
 * the state writes happen outside any derived; reads never sync (see annotations).
 */
export function startNotesOwnerSync(): () => void {
  return $effect.root(() => {
    $effect(() => {
      void authState.status;
      void authState.user;
      untrack(() => reader.syncNotes());
    });
  });
}
