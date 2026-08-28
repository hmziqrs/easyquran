<script lang="ts">
  import { page } from "$app/state";
  import { resolveChromeCopy } from "$lib/i18n/chrome-copy";
  import { marketingLocaleFromPath } from "$lib/i18n/marketing-copy";
  import MarketingFooter from "./_components/MarketingFooter.svelte";
  import MarketingHeader from "./_components/MarketingHeader.svelte";
  import MarketingTweaks from "./_components/MarketingTweaks.svelte";

  let { data, children } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  // Resolved once here and passed down. Every child resolving chrome itself rebuilt the same object
  // on every render, and pulled the resolver into its own module graph.
  const chrome = $derived(resolveChromeCopy(locale));
</script>

<div lang={locale} dir={chrome.direction} data-marketing-root>
  <a
    href="#main"
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded focus:bg-surface focus:px-3 focus:py-2 focus:text-body-s focus:text-foreground focus:shadow-md"
  >{chrome.skipToContent}</a>
  <MarketingHeader {locale} />
  <main id="main" tabindex="-1">{@render children()}</main>
  <MarketingFooter {locale} {chrome} owner={data.owner} year={data.year} />
  <MarketingTweaks {locale} triggerLabel={chrome.appearanceTrigger} />
</div>

<style>
  :global(body:has([data-marketing-root]) > a[href="#main"]) {
    display: none;
  }
</style>
