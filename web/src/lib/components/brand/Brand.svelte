<script lang="ts">
  import { cn } from "$lib/utils";
  import { SITE } from "$lib/config/site";
  import { publicHref } from "$lib/i18n/public-href";

  let {
    class: className = "",
    homeHref = "/",
    homeLabel = `${SITE.name} · home`,
  }: { class?: string; homeHref?: `/${string}`; homeLabel?: string } = $props();
</script>

<a
  class={cn("group inline-flex items-center gap-2.5", className)}
  href={publicHref(homeHref)}
  aria-label={homeLabel}
>
  <span
    class="size-2.5 rotate-45 rounded-[2px] bg-accent shadow-[0_0_0_3px_var(--accent-soft)] transition-transform duration-[400ms] ease-out group-hover:rotate-[135deg]"
    aria-hidden="true"
  ></span>
  <span class="font-mono text-sm tracking-tight"
    ><b class="font-medium">Easy</b><span class="font-normal text-fg-3">Quran</span></span
  >
</a>
