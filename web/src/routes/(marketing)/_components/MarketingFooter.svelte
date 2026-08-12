<script lang="ts">
  import { Footer } from "$lib/components/footer";
  import {
    marketingFooterLinks,
    resolveMarketingCopy,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";
  import type { OwnerPublic } from "$lib/types/owner";

  let {
    locale,
    owner,
    year,
  }: { locale: MarketingLocale; owner: OwnerPublic; year: number } = $props();

  const copy = $derived(resolveMarketingCopy(locale));
  const links = $derived(marketingFooterLinks(locale));
</script>

<Footer {owner} {year} copy={copy.footer} {links} />
