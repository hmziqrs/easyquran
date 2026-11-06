<script lang="ts">
  // Test host: VerseTools' tooltips need the provider the reader mounts above the verse list.
  import { TooltipProvider } from "#lib/components/ui/tooltip/index.js";
  import type { StackedTranslation } from "#lib/data/quran-types.js";
  import VerseTools from "../VerseTools.svelte";

  let {
    text,
    vKey,
    lanes = [],
  }: { text: string; vKey: string; lanes?: readonly StackedTranslation[] } = $props();
</script>

<TooltipProvider>
  <VerseTools {text} {vKey} {lanes} />
</TooltipProvider>
