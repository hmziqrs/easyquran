<script lang="ts">
  import { Footer } from "#lib/components/footer/index.js";
  import { marketingFooterLinks, type ChromeResolvedCopy } from "#lib/i18n/chrome-copy.js";
  import type { MarketingLocale } from "#lib/i18n/marketing-copy.js";
  import type { OwnerPublic } from "#lib/types/owner.js";

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
