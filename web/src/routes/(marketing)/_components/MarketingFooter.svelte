<script lang="ts">
  import { Footer } from "#lib/components/footer/index.js";
  import { marketingFooterLinks, type ChromeResolvedCopy } from "#lib/i18n/chrome-copy.js";
  import type { MarketingLocale } from "#lib/i18n/marketing-copy.js";

  let {
    locale,
    chrome,
    year,
  }: {
    locale: MarketingLocale;
    chrome: ChromeResolvedCopy;
    year: number;
  } = $props();

  const links = $derived(marketingFooterLinks(locale));
</script>

<Footer {year} copy={chrome.footer} {links} />
