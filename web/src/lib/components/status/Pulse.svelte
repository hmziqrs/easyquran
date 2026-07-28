<!--
  Pulse — the animated accent ping used for "live" indicators.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span class={cn("pulse", className)} aria-hidden="true"></span>

<style>
  .pulse {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 0 var(--accent);
    animation: var(--animate-pulse-ring);
  }
</style>
