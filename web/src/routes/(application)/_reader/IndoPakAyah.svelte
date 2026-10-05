<script lang="ts">
  import { indopakEnding } from "$lib/quran/view/indopak";
  import AyahOrnament from "./AyahOrnament.svelte";

  let { text, n, vKey }: { text: string; n: number; vKey: string } = $props();
  const ending = $derived(indopakEnding(text));
</script>

<span data-indopak-ayah>{ending.body}<span class="indopak-final-word">{ending.lastWord}<span class="indopak-ending" dir="rtl">{#if ending.sign}<span class="indopak-end-sign" dir="ltr"><span class:ruku={ending.ruku}>{ending.sign}</span>{#if ending.following}<span class="following">{ending.following}</span>{/if}</span>{/if}<span data-indopak-ornament><AyahOrnament {vKey} {n} /></span></span></span></span>

<style>
  @font-face {
    font-family: "IndoPak Reader Compat";
    src: url("/fonts/indopak-reader-compat-v2.woff2") format("woff2");
    font-weight: 400;
    font-display: block;
  }

  .indopak-ending {
    display: inline-block;
    position: relative;
    line-height: 1;
    unicode-bidi: isolate;
    white-space: nowrap;
  }

  .indopak-final-word {
    white-space: nowrap;
  }

  .indopak-end-sign {
    position: absolute;
    left: 50%;
    bottom: 0;
    display: flex;
    gap: 0.08em;
    transform: translateX(-50%);
    font-family: "IndoPak Reader Compat";
    font-size: 1em;
    line-height: 1;
    unicode-bidi: isolate;
  }

  .ruku {
    width: 0.211em;
  }

  .following {
    width: 0.227em;
  }
</style>
