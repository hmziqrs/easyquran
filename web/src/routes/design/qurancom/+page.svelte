<script lang="ts">
  import { describeDiff, type MirrorFontKey, type OurFontKey } from "./compare.ts";
  import type { PageData } from "./$types";

  let { data }: { data: PageData } = $props();

  const MIRROR_HAFS_STACK = '"Qurancom Hafs", "Qurancom Naskh", serif';
  const MIRROR_NASTALEEQ_STACK = '"Qurancom Nastaleeq", "Qurancom Naskh", serif';
  const MIRROR_NASKH_STACK = '"Qurancom Naskh", serif';
  const OUR_HAFS_STACK = '"KFGQPC Uthmanic Hafs", "Qurancom Naskh", serif';
  const OUR_INDOPAK_STACK = '"IndoPak Reader Compat", "Qurancom Nastaleeq", serif';

  function stackFor(key: MirrorFontKey | OurFontKey): string {
    if (key === "mirror-hafs") return MIRROR_HAFS_STACK;
    if (key === "mirror-nastaleeq") return MIRROR_NASTALEEQ_STACK;
    if (key === "mirror-naskh") return MIRROR_NASKH_STACK;
    if (key === "ours-hafs") return OUR_HAFS_STACK;
    return OUR_INDOPAK_STACK;
  }

  function captionFor(diff: PageData["pairs"][number]["verses"][number]["rows"][number]["diff"]) {
    if (diff === null) return "no diff computed";
    return describeDiff(diff);
  }
</script>

<svelte:head>
  <title>quran.com parity specimen</title>
  <meta name="robots" content="noindex,nofollow" />
</svelte:head>

<main class="specimen">
  <h1>quran.com parity specimen</h1>
  <p class="blurb">
    Dev-only side-by-side of the committed quran.com mirror (their font) and our provisioned
    DBs (our font). Comparison is codepoint-aligned on the stored text; the tajweed pair
    compares markup-stripped bare text because the markup formats differ by design.
    <a href="/design">Back to /design</a>
  </p>

  <form method="GET" action="/design/qurancom" class="controls">
    <label>
      Script
      <select name="script">
        <option value="all" selected={data.requestedScript === "all"}>all</option>
        {#each data.scripts as script (script.slug)}
          <option value={script.slug} selected={data.requestedScript === script.slug}>
            {script.slug}
          </option>
        {/each}
      </select>
    </label>
    <label>
      Verses
      <input name="verses" value={data.versesParam} />
    </label>
    <button type="submit">Compare</button>
  </form>

  {#each data.pairs as pair (pair.slug)}
    <section class="pair" aria-label={`${pair.slug} comparison`}>
      <h2>{pair.label}</h2>
      <p class="meta">
        mirror: {pair.mirrorFile} · {pair.bare ? "bare-text comparison (markup stripped)" : "stored-text comparison"}
      </p>
      {#if pair.verses.length === 0}
        <p class="caption">no requested verses exist in the mirror payload</p>
      {/if}
      {#each pair.verses as verse (verse.key)}
        <article class="verse">
          <h3>{verse.key}</h3>
          <div class="sides">
            <div class="side">
              <h4>quran.com (mirror)</h4>
              <p class="sample" style:font-family={stackFor(pair.mirrorFont)} dir="rtl" lang="ar">
                {verse.mirrorRaw}
              </p>
              {#if pair.bare}
                <p class="meta">bare text</p>
                <p class="sample" style:font-family={stackFor(pair.mirrorFont)} dir="rtl" lang="ar">
                  {verse.mirrorBare}
                </p>
              {/if}
            </div>
            {#each verse.rows as row (row.id)}
              <div class="side">
                <h4>ours ({row.label})</h4>
                {#if row.present}
                  <p class="sample" style:font-family={stackFor(pair.ourFont)} dir="rtl" lang="ar">
                    {row.raw}
                  </p>
                  {#if pair.bare}
                    <p class="meta">bare text</p>
                    <p class="sample" style:font-family={stackFor(pair.ourFont)} dir="rtl" lang="ar">
                      {row.bare}
                    </p>
                  {/if}
                  <p class="caption">{captionFor(row.diff)}</p>
                {:else}
                  <p class="caption">{row.note}</p>
                {/if}
              </div>
            {/each}
          </div>
        </article>
      {/each}
    </section>
  {/each}
</main>

<style>
  @font-face {
    font-family: "Qurancom Hafs";
    src: url("/qurancom/fonts/quran/hafs/uthmanic_hafs/UthmanicHafs1Ver18.woff2") format("woff2");
    font-weight: 400;
    font-display: block;
  }

  @font-face {
    font-family: "Qurancom Nastaleeq";
    src: url("/qurancom/fonts/quran/hafs/nastaleeq/indopak/indopak-nastaleeq-waqf-lazim-v4.2.1.woff2")
      format("woff2");
    font-weight: 400;
    font-display: block;
  }

  @font-face {
    font-family: "Qurancom Naskh";
    src: url("/qurancom/fonts/lang/arabic/NotoNaskhArabic-Regular.woff2") format("woff2");
    font-weight: 400;
    font-display: block;
  }

  @font-face {
    font-family: "KFGQPC Uthmanic Hafs";
    src: url("/fonts/uthmanic-hafs-v22.ttf") format("truetype");
    font-weight: 400;
    font-display: block;
  }

  @font-face {
    font-family: "IndoPak Reader Compat";
    src: url("/fonts/indopak-reader-compat-v4.woff2") format("woff2"),
      url("/fonts/indopak-reader-compat-v4.ttf") format("truetype");
    font-weight: 400;
    font-display: block;
    size-adjust: 125%;
  }

  .specimen {
    margin-inline: auto;
    max-width: 1100px;
    padding: 24px;
  }

  h1 {
    font-size: 28px;
    font-weight: 700;
  }

  h2 {
    font-size: 20px;
    font-weight: 700;
    margin-block: 32px 8px;
  }

  h3 {
    font-size: 15px;
    font-weight: 600;
    margin-block: 16px 8px;
  }

  h4 {
    font-size: 12px;
    font-weight: 600;
    margin-block: 8px 4px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .blurb,
  .meta {
    color: var(--fg-2);
    font-size: 13px;
    line-height: 1.5;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-block: 20px;
  }

  .controls label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .controls select,
  .controls input {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 6px;
  }

  .controls input {
    min-width: 320px;
  }

  .pair {
    border-top: 1px solid var(--border);
    margin-block: 28px;
    padding-block: 8px;
  }

  .verse {
    border: 1px solid var(--border);
    border-radius: 8px;
    margin-block: 12px;
    padding: 12px;
  }

  .sides {
    display: grid;
    gap: 12px;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  }

  .side {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
  }

  .sample {
    font-size: 28px;
    line-height: 2.1;
    min-height: 64px;
    padding-block: 8px;
  }

  .caption,
  .meta {
    font-size: 12px;
  }

  .caption {
    color: var(--fg-3);
    font-family: ui-monospace, monospace;
    overflow-wrap: anywhere;
  }
</style>
