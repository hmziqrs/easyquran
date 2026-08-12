<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/state";
  import { reader } from "$lib/stores/reader.svelte";
  import { toArabicDigits } from "$lib/data/quran";

  let {
    text,
    n,
    vKey,
    isTranslation,
    onToggleNote,
  }: {
    text: string;
    n: number;
    vKey: string;
    isTranslation?: boolean;
    onToggleNote?: () => void;
  } = $props();
  let Tools = $state<
    Component<{ text: string; vKey: string; onToggleNote?: () => void }> | null
  >(null);
  let hovered = $state(false);
  let focused = $state(false);
  let isDesktop = $state(false);

  const ayahId = $derived(`ayah-${vKey.replace(":", "-")}`);
  const isRevealed = $derived(page.url.hash === `#${ayahId}`);
  const translationActive = $derived(
    isTranslation ?? ("lang" in page.params && "translator" in page.params),
  );
  const noteOpen = $derived(reader.openNote === vKey);
  const showTools = $derived(!isDesktop || hovered || focused || noteOpen);

  onMount(() => {
    void import("./VerseTools.svelte")
      .then((module) => {
        Tools = module.default;
      })
      .catch(() => {});
    if (!browser) return;
    const mq = matchMedia("(min-width: 768px)");
    isDesktop = mq.matches;
    const update = () => {
      isDesktop = mq.matches;
    };
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  });
</script>

<li
  id={ayahId}
  data-verse-key={vKey}
  tabindex={showTools ? -1 : 0}
  class="verse-row group relative scroll-mt-24 border-b border-line px-5 pb-[22px] pt-[62px] transition-colors sm:px-9 {isRevealed
    ? 'revealed-ayah'
    : ''}"
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => (hovered = false)}
  onfocusin={() => (focused = true)}
  onfocusout={() => (focused = false)}
>
  {#if translationActive}
    <span
      lang={page.params.lang}
      dir="auto"
      class="verse-text verse-text--translation leading-[1.85] text-fg"
      style="font-size:var(--reader-translation-size, 1.0625rem)"
    >
      {text}<span class="ayah-marker translation-marker" data-verse-anchor={vKey}>{n}</span>
    </span>
  {:else}
    <span
      dir="rtl"
      lang="ar"
      class="verse-text font-arabic leading-[2.15] text-fg"
      style="font-size:var(--reader-arabic-size, 33px)"
    >
      {text}<span class="ayah-marker" data-verse-anchor={vKey}>{toArabicDigits(n)}</span>
    </span>
  {/if}

  {#if Tools && showTools}
    <Tools {text} {vKey} {onToggleNote} />
  {/if}
</li>

<style>
  .verse-text {
    display: block;
    text-align: right;
  }

  .verse-text--translation {
    font-family: var(--font-sans);
    text-align: start;
  }

  .translation-marker {
    font-family: var(--font-sans);
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-row {
    display: inline;
    padding: 0;
    border: 0;
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-text {
    display: inline;
  }
</style>
