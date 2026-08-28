<script lang="ts">
  import { cn } from "$lib/utils";
  import { SITE } from "$lib/config/site";
  import { publicHref } from "$lib/i18n/public-href";

  let {
    class: className = "",
    homeHref = "/",
    homeLabel = `${SITE.name} · home`,
  }: { class?: string; homeHref?: `/${string}`; homeLabel?: string } = $props();

  const brand = SITE.name.toLowerCase();
</script>

<!-- Board wordmark (design/PillLightCobalt.dc.html band 1): an 8px-radius ق holder on the
     primary fill + the split-tracking name — same mark MarketingHeader and the footer
     render inline. Replaces the rotated-diamond + mono lockup. -->
<a
  class={cn("group inline-flex items-center gap-2.5", className)}
  href={publicHref(homeHref)}
  aria-label={homeLabel}
>
  <span
    class="flex size-9 items-center justify-center rounded-sm bg-primary font-arabic text-[19px] font-bold leading-none text-primary-foreground"
    lang="ar"
    dir="rtl"
    aria-hidden="true">ق</span
  >
  <span class="text-[22px] font-extrabold tracking-[-0.035em] text-foreground"
    >{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span></span
  >
</a>
