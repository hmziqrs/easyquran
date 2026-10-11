<script lang="ts">
  import { Brand } from "#lib/components/brand/index.js";
  import { cn } from "#lib/utils.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    marketingHomeHref,
    marketingReaderHomeHref,
  } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";

  let {
    locale,
    readLabel,
    class: className = "",
  }: { locale: UiLocale; readLabel: string; class?: string } = $props();
</script>

<!-- Auth chrome is one slim row, matching the site Nav vocabulary: brand lockup left,
     a single reader exit right. Auth pages never carry a stacked identity block. -->
<header
  class={cn(
    "flex items-center justify-between gap-4 border-b border-border px-5 py-3 sm:px-10",
    className,
  )}
>
  <Brand homeHref={marketingHomeHref(locale)} />
  <a
    href={publicHref(marketingReaderHomeHref(locale))}
    class="inline-flex min-h-11 items-center rounded-pill px-3 text-sm font-medium text-fg-2 transition-colors hover:text-foreground"
    >{readLabel}</a
  >
</header>
