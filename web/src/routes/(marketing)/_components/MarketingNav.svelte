<script lang="ts">
  import { Nav } from "$lib/components/nav";
  import type { ChromeResolvedCopy } from "$lib/i18n/chrome-copy";
  import {
    marketingHomeHref,
    marketingLocaleLinks,
    marketingReaderHomeHref,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";

  let { locale, chrome }: { locale: MarketingLocale; chrome: ChromeResolvedCopy } = $props();

  const localeLinks = $derived(marketingLocaleLinks(locale));
</script>

<Nav
  copy={chrome.nav}
  brandCopy={chrome.brand}
  brandHomeHref={marketingHomeHref(locale)}
  searchHref={marketingReaderHomeHref(locale)}
  {localeLinks}
  direction={chrome.direction}
/>
