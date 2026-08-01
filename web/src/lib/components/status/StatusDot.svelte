<script lang="ts">
  import { cn } from "$lib/utils";
  let { class: className = "" }: { class?: string } = $props();
</script>

<span
  class={cn(
    "mr-1 inline-block size-2 shrink-0 grow-0 rounded-full bg-ok align-middle",
    "shadow-[0_0_0_3px_color-mix(in_oklab,var(--ok)_22%,transparent)]",
    className,
  )}
  aria-hidden="true"
></span>
