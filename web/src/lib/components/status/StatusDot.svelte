<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span
  class={cn(
    "ms-1 inline-block size-2 shrink-0 grow-0 rounded-full bg-success align-middle",
    "shadow-[0_0_0_3px_color-mix(in_oklab,var(--success)_22%,transparent)]",
    className,
  )}
  aria-hidden="true"
></span>
