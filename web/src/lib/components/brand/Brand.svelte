<script lang="ts">
  import { cn } from "#lib/utils.js";
  import { SITE } from "#lib/config/site.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import BrandMark from "./BrandMark.svelte";

  let {
    class: className = "",
    homeHref = "/",
    homeLabel = `${SITE.name} · home`,
    tone = "default",
  }: {
    class?: string;
    homeHref?: `/${string}`;
    homeLabel?: string;
    tone?: "default" | "on-accent";
  } = $props();

  const brand = SITE.name.toLowerCase();
  const onAccent = $derived(tone === "on-accent");
</script>

<a
  class={cn("group inline-flex items-center gap-2.5", className)}
  href={publicHref(homeHref)}
  aria-label={homeLabel}
>
  <span
    class={cn(
      "flex size-8 items-center justify-center rounded-sm",
      onAccent ? "bg-primary-foreground text-primary" : "bg-primary text-primary-foreground",
    )}
    aria-hidden="true"><BrandMark class="size-7" /></span
  >
  <span
    class={cn(
      "text-[20px] font-semibold tracking-[-0.025em]",
      onAccent ? "text-primary-foreground" : "text-foreground",
    )}
    >{#if onAccent}{brand}{:else}{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span
      >{/if}</span
  >
</a>
