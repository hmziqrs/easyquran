<script lang="ts">
  import { Nav } from "$lib/components/nav";
  import type { ChromeResolvedCopy } from "$lib/i18n/chrome-copy";
  import {
    marketingHomeHref,
    marketingLocaleLinks,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";

  let { locale, chrome }: { locale: MarketingLocale; chrome: ChromeResolvedCopy } = $props();

  const localeLinks = $derived(marketingLocaleLinks(locale));
</script>

<Nav
  copy={chrome.nav}
  brandCopy={chrome.brand}
  brandHomeHref={marketingHomeHref(locale)}
  {localeLinks}
  direction={chrome.direction}
/>
