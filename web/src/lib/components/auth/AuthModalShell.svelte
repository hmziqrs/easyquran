<script lang="ts">
  import type { Component } from "svelte";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { commandPalette } from "#lib/stores/command-palette.svelte.js";
  import { loadAuthModal } from "./auth-modal-loader";

  let Modal = $state<Component | null>(null);

  async function ensureLoaded(): Promise<void> {
    if (Modal) return;
    Modal = await loadAuthModal();
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
</script>

{#if Modal}
  <Modal />
{/if}
