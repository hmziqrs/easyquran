<script lang="ts">
  import { cn } from "$lib/utils";
  import { SITE } from "$lib/config/site";
  import { publicHref } from "$lib/i18n/public-href";

  let {
    class: className = "",
    homeHref = "/",
    homeLabel = `${SITE.name} · home`,
    tone = "default",
  }: {
    class?: string;
    homeHref?: `/${string}`;
    homeLabel?: string;
    /** `on-accent` renders the lockup for a primary-fill band: the holder flips to
        the on-primary pair so both marks stay legible on the accent surface (§5 D2). */
    tone?: "default" | "on-accent";
  } = $props();

  const brand = SITE.name.toLowerCase();
  const onAccent = $derived(tone === "on-accent");
</script>

<!-- Board wordmark (design/PillLightCobalt.dc.html band 1): an 8px-radius ق holder on
     the primary fill + the split-tracking name — same mark MarketingHeader and the footer
     render inline. Replaces the rotated-diamond + mono lockup. -->
<a
  class={cn("group inline-flex items-center gap-2.5", className)}
  href={publicHref(homeHref)}
  aria-label={homeLabel}
>
  <span
    class={cn(
      "flex size-8 items-center justify-center rounded-sm font-arabic text-[17px] font-bold leading-none",
      onAccent ? "bg-primary-foreground text-primary" : "bg-primary text-primary-foreground",
    )}
    lang="ar"
    dir="rtl"
    aria-hidden="true">ق</span
  >
  <span
    class={cn(
      "text-[20px] font-extrabold tracking-[-0.035em]",
      onAccent ? "text-primary-foreground" : "text-foreground",
    )}
    >{#if onAccent}{brand}{:else}{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span
      >{/if}</span
  >
</a>
