<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { Container } from "#lib/components/index.js";
  import AuthTopBar from "#lib/auth/components/AuthTopBar.svelte";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { protectedRouteRedirect } from "#lib/auth/route-guard.js";
  import { getAccountCopy } from "#lib/i18n/account-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { footer_read_quran } from "#lib/i18n/m/chrome.js";

  let { children } = $props();
  let ready = $state(false);

  const copy = getAccountCopy();
  const readQuranLabel = footer_read_quran(undefined, { locale: copy.locale });

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

<!-- Title lives here, not in +page.svelte: children are gated behind the session
     probe (`ready`), so a page-level title never reaches the SSR HTML — same
     reason the (auth) layout owns its titles (auth-page-meta.ts). -->
<svelte:head>
  <title>{copy.pageTitle}</title>
</svelte:head>

<div class="flex min-h-dvh flex-col bg-background" lang={copy.locale} dir={copy.direction}>
  <!-- The bar and the Read Quran exit sit outside the `ready` gate, so both are in the
       prerendered HTML and usable before the session probe settles. -->
  <AuthTopBar locale={copy.locale} readLabel={readQuranLabel} />

  <main id="main" tabindex="-1" class="flex flex-1 flex-col">
    <Container width="narrow" class="py-16">
      {#if ready}
        {@render children()}
      {:else}
        <p class="text-center text-sm text-fg-2" role="status" aria-live="polite">
          {copy.checkingSession}
        </p>
      {/if}
    </Container>
  </main>
</div>
