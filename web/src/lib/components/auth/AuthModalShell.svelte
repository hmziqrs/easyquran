<script lang="ts">
  import type { Component } from "svelte";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { signOutNoticeArmed } from "#lib/auth/sign-out-notice.svelte.js";
  import { commandPalette } from "#lib/stores/command-palette.svelte.js";
  import { loadAuthModal } from "./auth-modal-loader";

  let Modal = $state<Component | null>(null);
  let Notices = $state<Component | null>(null);

  async function ensureLoaded(): Promise<void> {
    if (Modal) return;
    Modal = await loadAuthModal();
  }

  async function ensureNoticesLoaded(): Promise<void> {
    if (Notices) return;
    // SAFETY: the Svelte compiler emits SyncNotices.svelte's default export as a Component.
    Notices = (await import("./SyncNotices.svelte")).default as Component;
  }

  $effect(() => {
    if (authModal.open) {
      commandPalette.hide();
      void ensureLoaded();
    }
  });

  $effect(() => {
    if (commandPalette.open) authModal.close();
  });

  // Sync notices (§9.5): the session-expired banner (EC-15/16/19) and the
  // sign-out unsynced-changes line (EC-14 notice). Lazy so the auth message
  // barrel stays out of the always-loaded chunk; once loaded it self-hides.
  $effect(() => {
    if (authState.sessionExpired || signOutNoticeArmed()) void ensureNoticesLoaded();
  });
</script>

{#if Modal}
  <Modal />
{/if}

{#if Notices}
  <Notices />
{/if}
