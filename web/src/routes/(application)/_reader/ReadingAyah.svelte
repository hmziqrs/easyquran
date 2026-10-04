<script lang="ts">
  import { page } from "$app/state";
  import { QuranScript, type QuranScript as QuranScriptValue } from "$lib/data/quran-types";
  import { parseTajweedSegments, tajweedRuleColor } from "$lib/quran/view/tajweed";
  import AyahOrnament from "./AyahOrnament.svelte";

  let {
    text,
    n,
    vKey,
    isTranslation = false,
    translationLang,
    pending = false,
    script = QuranScript.Uthmani,
  }: {
    text: string;
    n: number;
    vKey: string;
    isTranslation?: boolean;
    translationLang?: string;
    pending?: boolean;
    script?: QuranScriptValue;
  } = $props();

  const segments = $derived(script === QuranScript.Tajweed ? parseTajweedSegments(text) : null);
  const revealed = $derived(page.url.hash === `#ayah-${vKey.replace(":", "-")}`);
</script>

<span id="ayah-{vKey.replace(':', '-')}" data-verse-key={vKey} class:revealed-ayah={revealed}>
  {#if isTranslation}
    <span class="verse-text translation-text text-translation-foreground" lang={translationLang} dir="auto">
      {#if pending}<span class="pending" aria-hidden="true"></span>{:else}{text}{/if}<span class="ayah-marker" data-verse-anchor={vKey}>{n}</span>
    </span>
  {:else}
    <span class="verse-text arabic-text text-quran-foreground" dir="rtl" lang="ar">
      {#if segments}
        {#each segments as segment, index (index)}
          {#if segment.rule}<span style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span>{:else}{segment.text}{/if}
        {/each}
      {:else}
        {text}
      {/if}<AyahOrnament {vKey} {n} />
    </span>
  {/if}
</span>

<style>
  .arabic-text {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.15;
  }

  .translation-text {
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 1.0625rem);
    line-height: 1.85;
  }

  .translation-text:dir(rtl) {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--reader-translation-size, 1.0625rem) * 1.2);
    line-height: 2.05;
  }

  .ayah-marker {
    margin-inline-end: 0.55em;
  }

  .pending {
    background: var(--background-subtle);
    border-radius: 0.25rem;
    display: inline-block;
    height: 0.9em;
    vertical-align: middle;
    width: min(18rem, 60%);
  }
</style>
