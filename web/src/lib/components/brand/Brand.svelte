<!--
  Brand — the EasyQuran logo lockup: a rotated accent square with a soft
  glow, plus the wordmark. The dot is bespoke, so it lives in a scoped style.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import { SITE } from "$lib/config/site";
  let { class: className = "", href = "/" }: { class?: string; href?: string } = $props();
</script>

<a class={cn("brand", className)} {href} aria-label={`${SITE.name} · home`}>
  <span class="dot" aria-hidden="true"></span>
  <span class="font-mono text-sm tracking-tight"
    ><b class="font-medium">Easy</b><span class="tail">Quran</span></span
  >
</a>

<style>
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .dot {
    width: 10px;
    height: 10px;
    background: var(--accent);
    border-radius: 2px;
    box-shadow: 0 0 0 3px var(--accent-soft);
    transform: rotate(45deg);
    transition: transform 0.4s var(--ease-out);
  }
  .brand:hover .dot {
    transform: rotate(135deg);
  }
  .tail {
    color: var(--fg-3);
    font-weight: 400;
  }
</style>
