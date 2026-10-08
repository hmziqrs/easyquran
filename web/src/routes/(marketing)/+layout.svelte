<script lang="ts">
  import { page } from "$app/state";
  import { Nav } from "#lib/components/nav/index.js";
  import { resolveChromeCopy } from "#lib/i18n/chrome-copy.js";
  import { marketingHomeHref, marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { readerHrefFor, yoursPageHref } from "#lib/i18n/reader.js";
  import MarketingFooter from "./_components/MarketingFooter.svelte";
  import MarketingTweaks from "./_components/MarketingTweaks.svelte";

  let { data, children } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  // Resolved once here and passed down. Every child resolving chrome itself rebuilt the same object
  // on every render, and pulled the resolver into its own module graph.
  const chrome = $derived(resolveChromeCopy(locale));
  // One header everywhere: the marketing pages render the reader's Nav with the same four index
  // links, so the frame, links and controls never drift between the two surfaces.
  const indexLinks = $derived([
    { label: chrome.index.surahs, href: publicHref(readerHrefFor(locale, "/surah")) },
    { label: chrome.index.juz, href: publicHref(readerHrefFor(locale, "/juz")) },
    { label: chrome.index.pages, href: publicHref(readerHrefFor(locale, "/pages")) },
    { label: chrome.index.yours, href: publicHref(yoursPageHref()) },
  ]);
</script>

<div lang={locale} dir={chrome.direction} data-marketing-root>
  <a
    href="#main"
    class="sr-only focus:not-sr-only focus:absolute focus:start-3 focus:top-3 focus:z-[101] focus:rounded focus:bg-surface focus:px-3 focus:py-2 focus:text-body-s focus:text-foreground focus:shadow-md"
  >{chrome.skipToContent}</a>
  <Nav
    copy={chrome.nav}
    brandCopy={chrome.brand}
    brandHomeHref={marketingHomeHref(locale)}
    {indexLinks}
    direction={chrome.direction}
  />
  <main id="main" tabindex="-1">{@render children()}</main>
  <MarketingFooter {locale} {chrome} owner={data.owner} year={data.year} />
  <MarketingTweaks {locale} triggerLabel={chrome.appearanceTrigger} />
</div>

<style>
  :global(body:has([data-marketing-root]) > a[href="#main"]) {
    display: none;
  }
</style>
