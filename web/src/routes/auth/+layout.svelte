<script lang="ts">
  import { onMount } from "svelte";
  import { Brand } from "#lib/components/brand/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";
  import { footer_read_quran } from "#lib/i18n/m/chrome.js";
  import {
    marketingHomeHref,
    marketingReaderHomeHref,
  } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";

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
  <header class="flex items-center justify-between gap-4 border-b border-border px-5 py-3 sm:px-10">
    <Brand homeHref={marketingHomeHref(copy.locale)} />
    <a
      href={publicHref(marketingReaderHomeHref(copy.locale))}
      class="inline-flex min-h-11 items-center rounded-pill px-3 text-sm font-medium text-fg-2 transition-colors hover:text-foreground"
      >{readQuranLabel}</a
    >
  </header>
  <div class="flex flex-1 items-center justify-center px-5 py-10 sm:px-10">
    <main id="main" tabindex="-1" class="w-full max-w-[420px]">{@render children()}</main>
  </div>
</div>
