<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { Container } from "$lib/components";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { installPurgeHook } from "$lib/auth/purge-hook";
  import { guestOnlyRedirect } from "$lib/auth/route-guard";

  let { children } = $props();
  let ready = $state(false);

  async function hydrateRouteAuth(): Promise<void> {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
    await authState.probe();
    const target = guestOnlyRedirect(page.url.pathname, authState.status, authState.user);
    if (target) {
      await goto(resolve(target), { replaceState: true });
      ready = true;
      return;
    }
    ready = true;
  }

  onMount(() => {
    void hydrateRouteAuth();
  });
</script>

<Container width="narrow" class="flex min-h-[70vh] flex-col justify-center py-16">
  <!-- `main#main` renders unconditionally: the root layout's skip link targets #main, and
       gating it behind `ready` left prerendered auth pages without the anchor, failing the
       build with `no element with id="main" exists`. -->
  <main id="main" class="mx-auto w-full max-w-[420px]">
    {#if ready}
      {@render children()}
    {:else}
      <p class="text-center text-sm text-fg-2" role="status" aria-live="polite">Checking your session…</p>
    {/if}
  </main>
</Container>
