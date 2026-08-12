<script lang="ts">
  import { page } from "$app/state";
  import {
    marketingLocaleFromPath,
    resolveMarketingCopy,
  } from "$lib/i18n/marketing-copy";
  import MarketingFooter from "./_components/MarketingFooter.svelte";
  import MarketingNav from "./_components/MarketingNav.svelte";
  import MarketingTweaks from "./_components/MarketingTweaks.svelte";

  let { data, children } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveMarketingCopy(locale));
</script>

<div lang={locale} dir={copy.direction} data-marketing-root>
  <a
    href="#main"
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded focus:bg-bg-1 focus:px-3 focus:py-2 focus:text-sm focus:text-fg focus:shadow-lg"
  >{copy.skipToContent}</a>
  <MarketingNav {locale} />
  <main id="main" tabindex="-1">{@render children()}</main>
  <MarketingFooter {locale} owner={data.owner} year={data.year} />
  <MarketingTweaks {locale} />
</div>

<style>
  :global(body:has([data-marketing-root]) > a[href="#main"]) {
    display: none;
  }
</style>
