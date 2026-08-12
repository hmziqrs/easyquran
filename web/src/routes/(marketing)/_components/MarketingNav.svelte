<script lang="ts">
  import { Nav } from "$lib/components/nav";
  import {
    marketingHomeHref,
    marketingLocaleLinks,
    marketingReaderHomeHref,
    resolveMarketingCopy,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";

  let { locale }: { locale: MarketingLocale } = $props();

  const copy = $derived(resolveMarketingCopy(locale));
  const localeLinks = $derived(marketingLocaleLinks(locale));
</script>

<Nav
  copy={copy.nav}
  brandCopy={copy.brand}
  brandHomeHref={marketingHomeHref(locale)}
  searchHref={marketingReaderHomeHref(locale)}
  {localeLinks}
  direction={copy.direction}
/>
