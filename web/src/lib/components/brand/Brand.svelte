<!--
  Brand — the EasyQuran logo lockup: a rotated accent square with a soft
  glow, plus the wordmark. The square rotates further on hover, driven by
  `group-hover` on the anchor.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import { SITE } from "$lib/config/site";
  let { class: className = "", href = "/" }: { class?: string; href?: string } = $props();
</script>

<a
  class={cn("group inline-flex items-center gap-2.5", className)}
  {href}
  aria-label={`${SITE.name} · home`}
>
  <span
    class="size-2.5 rotate-45 rounded-[2px] bg-accent shadow-[0_0_0_3px_var(--accent-soft)] transition-transform duration-[400ms] ease-out group-hover:rotate-[135deg]"
    aria-hidden="true"
  ></span>
  <span class="font-mono text-sm tracking-tight"
    ><b class="font-medium">Easy</b><span class="font-normal text-fg-3">Quran</span></span
  >
</a>
