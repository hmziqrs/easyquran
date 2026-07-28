<!--
  Eyebrow — the small uppercase mono label that opens most sections.
  Uses the global `eyebrow` utility (see layout.css).
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  let { class: className = "", children }: { class?: string; children: Snippet } = $props();
</script>

<div class={cn("eyebrow", className)}>{@render children()}</div>
