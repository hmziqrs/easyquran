<script lang="ts">
  import { onMount } from "svelte";
  import { indopakEnding, type IndoPakEndAnnotation } from "$lib/quran/view/indopak";
  import { indopakFont, loadIndopakFont } from "$lib/quran/view/indopak-font.svelte";
  import AyahOrnament from "./AyahOrnament.svelte";

  let { text, n, vKey }: { text: string; n: number; vKey: string } = $props();
  const ending = $derived(indopakEnding(text));
  const hidden = $derived(indopakFont.status === "loading" || indopakFont.status === "error");
  onMount(() => { void loadIndopakFont(); });

  function em(value: number | undefined): string | undefined {
    if (value === undefined) return undefined;
    return `${value}em`;
  }

  function stackPadding(rows: number): string | undefined {
    if (rows < 2) return undefined;
    return em((rows - 1) * 0.36);
  }
</script>

<span data-indopak-ayah style:visibility={hidden ? "hidden" : undefined} aria-hidden={hidden || undefined}>{#each ending.words as word (word.offset)}<span class={["indopak-word", word.stop && "indopak-stop"]}>{#each word.parts as part (part.offset)}{#if part.annotations.length}<span class="indopak-body-sign" dir="ltr">{@render signs(part.annotations)}</span>{:else}{part.text}{/if}{/each}</span>{word.gap}{/each}<span class="indopak-final-word" style:padding-block-start={stackPadding(ending.annotations.length)}>{ending.lastWordText}{#if ending.lastWordSpace}<span class="indopak-space">{ending.lastWordSpace}</span>{/if}<span class="indopak-ending" dir="rtl">{#if ending.sign}<span class="indopak-end-sign" dir="ltr">{@render signs(ending.annotations)}</span>{/if}<span data-indopak-ornament><AyahOrnament {vKey} {n} digits="urdu" /></span></span></span></span>{#snippet signs(annotations: readonly IndoPakEndAnnotation[])}{#each annotations as annotation (annotation.offset)}<span style:width={em(annotation.widthEm)} style:text-indent={em(annotation.indentEm)}>{annotation.mark}{#if annotation.space}<span class="indopak-space">{annotation.space}</span>{/if}</span>{/each}{/snippet}

<style>
  @font-face {
    font-family: "IndoPak Reader Compat";
    src: url("/fonts/indopak-reader-compat-v4.woff2") format("woff2"),
      url("/fonts/indopak-reader-compat-v4.ttf") format("truetype");
    font-weight: 400;
    font-display: block;
    size-adjust: 125%;
  }

  .indopak-word,
  .indopak-final-word {
    display: inline-block;
    white-space: nowrap;
  }

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

  .indopak-end-sign {
    position: absolute;
    pointer-events: none;
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
    text-align: start;
    text-align-last: auto;
  }

  .indopak-end-sign > :global(span) {
    height: 0.36em;
    line-height: 0;
  }

  .indopak-space {
    font-size: 0;
  }

  .indopak-body-sign {
    display: inline-flex;
    gap: 0.16em;
    unicode-bidi: isolate;
    white-space: nowrap;
    text-align: start;
    text-align-last: auto;
  }

  [data-indopak-ornament] :global(.ayah-ornament) {
    direction: rtl;
    color: inherit;
    font-family: "IndoPak Reader Compat";
    font-size: 0.68em;
    margin-inline-end: 0;
  }

  .indopak-final-word {
    margin-inline-end: 0.08em;
  }
</style>
