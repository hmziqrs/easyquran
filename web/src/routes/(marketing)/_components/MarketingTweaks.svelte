<script lang="ts">
  import { Tweaks } from "$lib/components/tweaks";
  import {
    resolveMarketingCopy,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";

  let { locale }: { locale: MarketingLocale } = $props();

  const copy = $derived(resolveMarketingCopy(locale));
</script>

<Tweaks copy={copy.tweaks} showReaderTools={false} />
