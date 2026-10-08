<script lang="ts">
  import { reader } from "#lib/stores/reader.svelte.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { positionLabel } from "./position-label";

  let {
    /** SSR-known position for the route's first paint; hydration takes over from the store. */
    initial,
  }: {
    initial: { globalPage: number; juz?: number | null; hizb?: number | null };
  } = $props();

  const copy = getReaderUiCopy();

  const position = $derived(reader.position ?? initial);
</script>

<!-- Plain live text, quran.com's page-info pattern — not links. Inherits shell direction. -->
<span class="min-w-0 truncate text-[12px] tabular-nums text-muted">{positionLabel(copy, position)}</span>
