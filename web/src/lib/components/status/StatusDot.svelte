<!--
  StatusDot — the small "available / ok" indicator dot with a soft glow ring.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span class={cn("status-dot", className)} aria-hidden="true"></span>

<style>
  .status-dot {
    display: inline-block;
    flex: 0 0 auto;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--ok) 22%, transparent);
    margin-right: 4px;
    vertical-align: middle;
  }
</style>
