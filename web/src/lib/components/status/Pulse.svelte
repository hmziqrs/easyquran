<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span
  class={cn(
    "inline-block size-1.5 animate-pulse-ring rounded-full bg-accent shadow-[0_0_0_0_var(--accent)]",
    className,
  )}
  aria-hidden="true"
></span>
