<script lang="ts">
  import { onMount } from "svelte";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import AuthTopBar from "#lib/auth/components/AuthTopBar.svelte";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";
  import { footer_read_quran } from "#lib/i18n/m/chrome.js";

  let { children } = $props();

  const copy = getAuthCopy();
  const readQuranLabel = footer_read_quran(undefined, { locale: copy.locale });

  onMount(() => {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
  });
</script>

<!-- Callback pages (success/failure) used to be chromeless: a user landing on a failure
     had no route back into the product besides the form's own link. The bar renders in the
     popup variant too (harmless — the popup closes itself), and the exit exists in the
     prerendered failure HTML before JS runs. -->
<div class="flex min-h-dvh flex-col bg-background" lang={copy.locale} dir={copy.direction}>
  <AuthTopBar locale={copy.locale} readLabel={readQuranLabel} />
  <div class="flex flex-1 items-center justify-center px-5 py-10 sm:px-10">
    <main id="main" tabindex="-1" class="w-full max-w-[420px]">{@render children()}</main>
  </div>
</div>
