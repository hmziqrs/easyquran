<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { toArabicDigits } from "$lib/data/quran";
  import { reader } from "$lib/stores/reader.svelte";

  let { text, n, vKey }: { text: string; n: number; vKey: string } = $props();
  let Tools = $state<Component<{ text: string; vKey: string }> | null>(null);

  onMount(() => {
    void import("./VerseTools.svelte").then((module) => {
      Tools = module.default;
    });
  });
</script>

<li
  id="ayah-{vKey.replace(":", "-")}"
  data-verse-key={vKey}
  class="verse-row group relative scroll-mt-24 border-b border-line px-5 pb-[22px] pt-[62px] transition-colors sm:px-9"
>
  <span
    dir="rtl"
    class="verse-text font-arabic leading-[2.15] text-fg"
    style="font-size:{reader.arabicSizePx}"
  >
    {text}<span class="ayah-marker">{toArabicDigits(n)}</span>
  </span>

  {#if Tools}
    <Tools {text} {vKey} />
  {/if}
</li>

<style>
  :global([data-reader-mode="reading"]) .verse-row {
    display: inline;
    padding: 0;
    border: 0;
  }

  :global([data-reader-mode="reading"]) .verse-text {
    display: inline;
  }
</style>
