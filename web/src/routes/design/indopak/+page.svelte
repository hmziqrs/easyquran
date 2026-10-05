<script lang="ts">
  import { onMount } from "svelte";
  import { QuranScript } from "$lib/data/quran-types";
  import { loadArabicFont } from "$lib/fonts/arabic-fonts";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import ReadingAyah from "../../(application)/_reader/ReadingAyah.svelte";
  import type { PageData } from "./$types";

  let { data }: { data: PageData } = $props();
  let size = $state(33);
  let width = $state(640);
  let ready = $state(false);

  onMount(() => {
    ready = true;
    void loadArabicFont("kfgqpc-hafs");
  });
</script>

<svelte:head>
  <title>IndoPak font compatibility specimen</title>
  <meta name="robots" content="noindex,nofollow" />
</svelte:head>

<section aria-label="Font compatibility specimen" class="specimen-page" style:--reader-arabic-size={`${size}px`}>
  <h1>IndoPak font compatibility specimen</h1>
  <p>Packaged compatibility font, original text, and end signs anchored to the ayah ornament.</p>
  <p>
    Nine private codes, 1,383 occurrences. {data.contextCount} context classes across
    {data.specimens.length} actual verses. Original DB strings pass unchanged to ReadingAyah.
  </p>
  <div class="controls">
    <label>Font size
      <select bind:value={size} disabled={!ready}>
        {#each [24, 33, 48] as value (value)}<option value={value}>{value}px</option>{/each}
      </select>
    </label>
    <label>Run width
      <select bind:value={width} disabled={!ready}>
        {#each [320, 640, 960] as value (value)}<option value={value}>{value}px</option>{/each}
      </select>
    </label>
  </div>
  <ul>
    {#each data.symbols as symbol (symbol.codepoint)}
      <li>{symbol.codepoint} — {symbol.meaning} ({symbol.occurrences})</li>
    {/each}
  </ul>
  <section aria-label="Unicode property comparison" class="compat" style:max-width={`${width}px`}>
    <h2>17:7: original versus diagnostic comparison</h2>
    <p>Comparison changes only E004 to U+0657 in this development view. Never reader data.</p>
    <p data-diagnostic="original" class="diagnostic" dir="rtl" lang="ar">{data.diagnostic.original}</p>
    <p data-diagnostic="unicode" class="diagnostic" dir="rtl" lang="ar">{data.diagnostic.comparison}</p>
    <p class="diagnostic" dir="rtl" lang="ar">(١٢٣) ٤٥٦ — ۱۲۳ (٤٥٦)</p>
  </section>
  <TooltipProvider>
    <section aria-label="Unaffected controls">
      <h2>Controls</h2>
      {#each data.controls as control (`${control.script}:${control.key}`)}
        <div
          class="sample"
          class:compat={control.script === QuranScript.IndoPak}
          class:uthmani={control.script === QuranScript.Uthmani}
          style:max-width={`${width}px`}
          data-control={`${control.script}:${control.key}`}
        >
          <h3>{control.script} {control.key}</h3>
          <div class="run" dir="rtl">
            <ReadingAyah text={control.text} n={Number(control.key.split(":")[1])} vKey={`control-${control.script}-${control.key}`} script={control.script} />
          </div>
        </div>
      {/each}
    </section>
    <section aria-label="All private character contexts">
      <h2>Actual IndoPak verses</h2>
      {#each data.specimens as specimen (specimen.key)}
        <article class="sample compat" style:max-width={`${width}px`} data-specimen={specimen.key}>
          <h3>{specimen.key}</h3>
          <p class="context">{specimen.labels.join(" · ")}</p>
          <div class="run" dir="rtl">
            <ReadingAyah text={specimen.text} n={specimen.ayah} vKey={specimen.key} script={QuranScript.IndoPak} />
          </div>
        </article>
      {/each}
    </section>
  </TooltipProvider>
</section>

<style>
  @font-face {
    font-family: "IndoPak Reader Compat";
    src: url("/fonts/indopak-reader-compat-v2.woff2") format("woff2");
    font-weight: 400;
    font-display: block;
  }

  .specimen-page {
    padding: 24px;
  }

  h1, h2, h3 {
    font-weight: 700;
    margin-block: 20px 12px;
  }

  .controls {
    display: flex;
    gap: 24px;
    flex-wrap: wrap;
    margin-block: 24px;
  }

  select {
    border: 1px solid currentColor;
    margin-inline-start: 8px;
    padding: 4px;
  }

  .compat {
    --reader-arabic-family: "IndoPak Reader Compat";
  }

  .uthmani {
    --reader-arabic-family: "KFGQPC Uthmanic Hafs";
  }

  .sample {
    border: 1px solid var(--border);
    margin-block: 24px;
    padding: 16px;
  }

  .context {
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .run, .diagnostic {
    padding-block: 20px;
  }

  .diagnostic {
    font-family: var(--reader-arabic-family);
    font-size: var(--reader-arabic-size);
    line-height: 2.15;
  }
</style>
