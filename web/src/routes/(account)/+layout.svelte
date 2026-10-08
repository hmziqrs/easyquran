<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { Container } from "#lib/components/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { protectedRouteRedirect } from "#lib/auth/route-guard.js";
  import { publicHref } from "#lib/i18n/public-href.js";

  let { children } = $props();
  let ready = $state(false);

  async function hydrateRouteAuth(): Promise<void> {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
    await authState.probe();
    const target = protectedRouteRedirect(authState.status);
    if (target) {
      await goto(publicHref(target), { replace: true });
      return;
    }
    ready = true;
  }

  onMount(() => {
    void hydrateRouteAuth();
  });
</script>

<main id="main" tabindex="-1">
  <Container width="narrow" class="py-16">
    {#if ready}
      {@render children()}
    {:else}
      <p class="text-center text-sm text-fg-2" role="status" aria-live="polite">Checking your session…</p>
    {/if}
  </Container>
</main>
