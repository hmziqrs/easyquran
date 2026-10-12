<script lang="ts">
  import type { Component } from "svelte";
  import { untrack } from "svelte";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { signOutNoticeArmed } from "#lib/auth/sign-out-notice.svelte.js";
  import { commandPalette } from "#lib/stores/command-palette.svelte.js";
  import { loadAuthModal } from "./auth-modal-loader";
  import { mergeReview } from "./merge-review.svelte";

  let Modal = $state<Component | null>(null);
  let MergeModal = $state<Component | null>(null);
  let Notices = $state<Component | null>(null);

  async function ensureLoaded(): Promise<void> {
    if (Modal) return;
    Modal = await loadAuthModal();
  }

  async function ensureMergeLoaded(): Promise<void> {
    if (MergeModal) return;
    // SAFETY: the Svelte compiler emits MergeReviewDialog.svelte's default export as a Component.
    MergeModal = (await import("./MergeReviewDialog.svelte")).default as Component;
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

  // Merge review (§9.4): detection runs on every authenticated edge — login,
  // probe resolve, account switch. Nothing is enqueued until an explicit choice.
  $effect(() => {
    const user = authState.user;
    if (authState.status !== "authenticated" || user === null) return;
    untrack(() => {
      mergeReview.evaluate(user.id);
      if (mergeReview.open) void ensureMergeLoaded();
    });
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

{#if MergeModal}
  <MergeModal />
{/if}

{#if Notices}
  <Notices />
{/if}
