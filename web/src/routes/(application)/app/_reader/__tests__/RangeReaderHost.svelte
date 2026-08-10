<script lang="ts">
  import { untrack } from "svelte";
  import RangeReader from "../RangeReader.svelte";
  import type { RangePageData } from "$lib/data/quran-types";

  let {
    initial,
    expose,
  }: {
    initial: RangePageData;
    expose: (setter: (next: RangePageData) => void) => void;
  } = $props();

  // Svelte 5 dropped imperative $set on mounted instances, so this host hands
  // the test a setter that drives a reactive `data` prop update on the living
  // RangeReader — the same way SvelteKit feeds a route new `data` on navigation.
  // The coordinator instance is created once in RangeReader init and persists
  // across these updates, so the route-key guard is exercised on one instance.
  // untrack + closures: we deliberately seed from `initial` and register `expose`
  // exactly once; only the setter drives subsequent updates.
  let current = $state<RangePageData>(untrack(() => initial));
  untrack(() =>
    expose((next: RangePageData) => {
      current = next;
    }),
  );
</script>

<RangeReader data={current} />
