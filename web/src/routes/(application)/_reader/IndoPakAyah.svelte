<script lang="ts">
  import { indopakEnding, type IndoPakEndAnnotation } from "$lib/quran/view/indopak";
  import AyahOrnament from "./AyahOrnament.svelte";

  let { text, n, vKey }: { text: string; n: number; vKey: string } = $props();
  const ending = $derived(indopakEnding(text));

  // Quran.com's layout: every word is one unbreakable box, so lines break only at the source
  // whitespace between boxes and pause signs stay with their word. Text nodes stay exact.
  function em(value: number | undefined): string | undefined {
    if (value === undefined) return undefined;
    return `${value}em`;
  }
</script>

<span data-indopak-ayah>{#each ending.words as word (word.offset)}<span class={["indopak-word", word.stop && "indopak-stop"]}>{#each word.parts as part (part.offset)}{#if part.annotations.length}<span class="indopak-body-sign" dir="ltr">{@render signs(part.annotations)}</span>{:else}{part.text}{/if}{/each}</span>{word.gap}{/each}<span class="indopak-final-word">{ending.lastWordText}{#if ending.lastWordSpace}<span class="indopak-space">{ending.lastWordSpace}</span>{/if}<span class="indopak-ending" dir="rtl">{#if ending.sign}<span class="indopak-end-sign" dir="ltr">{@render signs(ending.annotations)}</span>{/if}<span data-indopak-ornament><AyahOrnament {vKey} {n} digits="urdu" /></span></span></span></span>{#snippet signs(annotations: readonly IndoPakEndAnnotation[])}{#each annotations as annotation (annotation.offset)}<span style:width={em(annotation.widthEm)} style:text-indent={em(annotation.indentEm)}>{annotation.mark}{#if annotation.space}<span class="indopak-space">{annotation.space}</span>{/if}</span>{/each}{/snippet}

<style>
  @font-face {
    font-family: "IndoPak Reader Compat";
    src: url("/fonts/indopak-reader-compat-v4.woff2") format("woff2");
    font-weight: 400;
    font-display: block;
    /* Lateef draws small on its em; 125% matches Quran.com's IndoPak lettering per px. */
    size-adjust: 125%;
  }

  .indopak-word,
  .indopak-final-word {
    display: inline-block;
    white-space: nowrap;
  }

  /* Quran.com: 13px instead of 6px after a word carrying a stop sign at its 26px text size. */
  .indopak-stop {
    margin-inline-end: 0.27em;
  }

  .indopak-ending {
    display: inline-block;
    position: relative;
    line-height: 1;
    unicode-bidi: isolate;
    white-space: nowrap;
  }

  /* End signs stack above the ring, first sign nearest it, as Quran.com stacks them. */
  .indopak-end-sign {
    position: absolute;
    left: 50%;
    bottom: 0.17em;
    display: flex;
    flex-direction: column-reverse;
    align-items: center;
    transform: translateX(-50%);
    font-family: "IndoPak Reader Compat";
    font-size: 1em;
    line-height: 1;
    unicode-bidi: isolate;
  }

  /* The font draws signs raised; compact rows keep stacked signs one sign-height apart. */
  .indopak-end-sign > :global(span) {
    height: 0.36em;
    line-height: 0;
  }

  /* Source whitespace inside a sign cluster or before the ayah sign: kept, drawn zero-width. */
  .indopak-space {
    font-size: 0;
  }

  .indopak-body-sign {
    display: inline-flex;
    gap: 0.16em;
    unicode-bidi: isolate;
    white-space: nowrap;
  }

  /* Plain circular marker with Urdu-form numerals, drawn by the same font and in the text
     colour, as in IndoPak mushafs and Quran.com. */
  [data-indopak-ornament] :global(.ayah-ornament) {
    color: inherit;
    font-family: "IndoPak Reader Compat";
    font-size: 0.68em;
    /* The gap after the ayah moves to the final word, so end signs centre on the ring. */
    margin-inline-end: 0;
  }

  .indopak-final-word {
    margin-inline-end: 0.08em;
  }
</style>
