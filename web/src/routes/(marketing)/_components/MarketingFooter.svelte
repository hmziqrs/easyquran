<script lang="ts">
  import { Footer } from "$lib/components/footer";
  import { marketingFooterLinks, type ChromeResolvedCopy } from "$lib/i18n/chrome-copy";
  import type { MarketingLocale } from "$lib/i18n/marketing-copy";
  import type { OwnerPublic } from "$lib/types/owner";

  let {
    locale,
    chrome,
    owner,
    year,
  }: {
    locale: MarketingLocale;
    chrome: ChromeResolvedCopy;
    owner: OwnerPublic;
    year: number;
  } = $props();

  const links = $derived(marketingFooterLinks(locale));
</script>

<Footer {owner} {year} copy={chrome.footer} {links} />
