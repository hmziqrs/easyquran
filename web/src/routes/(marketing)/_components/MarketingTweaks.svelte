<script lang="ts">
  import { Tweaks } from "$lib/components/tweaks";
  import type { MarketingLocale } from "$lib/i18n/marketing-copy";

  let { locale, triggerLabel }: { locale: MarketingLocale; triggerLabel: string } = $props();

  // Dynamic import on purpose: the appearance panel owns the largest block of chrome copy and
  // nothing renders it until the user opens the panel. See docs/i18n-bundle-plan.md.
  const loadCopy = async () => {
    const { resolveAppearanceCopy } = await import("$lib/i18n/appearance-copy");
    return resolveAppearanceCopy(locale);
  };
</script>

<Tweaks {triggerLabel} {loadCopy} showReaderTools={false} />
