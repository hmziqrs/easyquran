<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span
  class={cn(
    "inline-block size-1.5 animate-pulse-ring rounded-full bg-primary shadow-[0_0_0_0_var(--primary)]",
    className,
  )}
  aria-hidden="true"
></span>
