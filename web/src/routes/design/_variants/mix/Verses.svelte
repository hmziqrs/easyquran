<script lang="ts">
  import Bismillah from "$lib/components/brand/Bismillah.svelte";
  import { Icon, type IconName } from "$lib/components/icon";
  import { toArabicDigits } from "$lib/data/quran";
  import type { StackId } from "./axes";
  import type { Tweaks } from "./tweaks";
  import type { MixTranslation } from "./types";

  let {
    stack,
    mode,
    surahNum,
    opener,
    verses,
    translations,
    tweaks,
  }: {
    stack: StackId;
    mode: "verse" | "reading";
    surahNum: number;
    opener: string | null;
    verses: readonly string[];
    translations: readonly MixTranslation[];
    tweaks: Tweaks;
  } = $props();

  // Non-functional copies of the live verse toolbar, so every stack variant keeps the same tools.
  const TOOLS: readonly { icon: IconName; label: string }[] = [
    { icon: "bookmark", label: "Bookmark ayah" },
    { icon: "copy", label: "Copy ayah" },
    { icon: "share", label: "Share ayah" },
    { icon: "note", label: "Open note and tafsir" },
  ];

  // Lane edges cycle the hue set, starting off the cobalt the ayah ornament already wears.
  const LANE = [
    "var(--hue-2-legible)",
    "var(--hue-3-legible)",
    "var(--hue-4-legible)",
    "var(--hue-1-legible)",
  ] as const;

  function laneColor(index: number): string {
    return LANE[index % LANE.length] ?? LANE[0];
  }

  function nameOf(t: MixTranslation): string {
    return tweaks.names === "full" ? t.translator : t.name;
  }

  let openAll = $state(false);
  const lead = $derived(translations[0]);
  const others = $derived(translations.slice(1));
  const columns = $derived(`repeat(${Math.max(1, translations.length)}, minmax(0, 1fr))`);
  // Catalogue names are the short ones (Pickthall, جالندہری); full translator names run long.
  const otherNames = $derived(others.map((t) => t.name).join(", "));
  const showKey = $derived(stack === "c" && tweaks.label === "key" && translations.length > 0);
</script>

{#snippet label(t: MixTranslation, color: string | undefined)}
  <!-- A label stays on its translation's own starting edge: the right for Urdu. -->
  <span
    dir="auto"
    class="tr-label"
    style:text-align={t.direction === "rtl" ? "right" : "left"}
    style:color={color}
  >
    {nameOf(t)}<span class="tr-label-lang">{` · ${t.language}`}</span>
  </span>
{/snippet}

{#snippet quietBlock(t: MixTranslation, i: number)}
  <div dir={t.direction} lang={t.languageCode} class={["tr-block", t.direction === "rtl" && "tr-block--rtl"]}>
    {@render label(t, undefined)}
    <p class={["tr-text", t.direction === "rtl" && "tr-rtl"]}>{t.texts[i]}</p>
  </div>
{/snippet}

{#snippet laneBlock(t: MixTranslation, i: number, k: number)}
  {@const color = laneColor(k)}
  <div
    dir={t.direction}
    lang={t.languageCode}
    class={["tr-block lane", t.direction === "rtl" && "tr-block--rtl"]}
    style:--lane={color}
  >
    {#if tweaks.label === "above"}
      {@render label(t, color)}
    {/if}
    <p class={["tr-text", t.direction === "rtl" && "tr-rtl"]}>
      {#if tweaks.label === "inline"}
        <span class="lane-name" dir={t.direction}><bdi
            class={[/[\u0600-\u06FF]/.test(nameOf(t)) && "lane-name--arabic"]}
            style:color={color}>{nameOf(t)}</bdi
          ></span>
      {:else if tweaks.label === "key"}
        <span class="sr-only">{nameOf(t)}:</span>
      {/if}
      {t.texts[i]}
    </p>
  </div>
{/snippet}

{#if mode === "reading"}
  <div class="reading-block">
    {#if opener}
      <div class="flex justify-center pb-3">
        <Bismillah class="w-44 text-quran-foreground" title={opener} />
      </div>
    {/if}
    <p dir="rtl" lang="ar" class="reading-flow">
      {#each verses as ayah, i (i)}{ayah}<span class="ayah-ornament">&#x06DD;{toArabicDigits(i + 1)}</span>
        {" "}{/each}
    </p>
  </div>
{:else}
  {#if opener}
    <div class="flex justify-center py-11">
      <Bismillah class="w-44 text-quran-foreground" title={opener} />
    </div>
  {/if}

  {#if showKey}
    <div class="list-head flex flex-wrap items-center gap-x-5 gap-y-2">
      <span class="text-[12.5px] font-semibold text-foreground-secondary">Key</span>
      {#each translations as t, k (t.id)}
        <span class="flex items-center gap-2 text-[13.5px] text-foreground">
          <span class="h-4 w-[3px] rounded-full" style:background={laneColor(k)} aria-hidden="true"></span>
          <span dir="auto">{nameOf(t)}</span>
          <span class="text-foreground-secondary">{t.language}</span>
        </span>
      {/each}
    </div>
  {/if}

  {#if translations.length > 1 && stack === "d"}
    <div class="list-head list-head--sticky hidden gap-8 lg:grid" style:grid-template-columns={columns}>
      {#each translations as t (t.id)}
        <span
          dir="auto"
          class="truncate text-[13px] font-semibold text-foreground"
          style:text-align={t.direction === "rtl" ? "right" : "left"}
        >
          {nameOf(t)} <span class="font-normal text-foreground-secondary">· {t.language}</span>
        </span>
      {/each}
    </div>
  {/if}

  {#if others.length > 0 && stack === "e"}
    <div class="list-head flex items-center justify-between gap-3">
      <span class="text-[13.5px] text-foreground-secondary">
        Reading <span class="font-semibold text-foreground" dir="auto">{lead?.name}</span>; comparing
        <span dir="auto">{otherNames}</span>
      </span>
      <button
        type="button"
        aria-pressed={openAll}
        onclick={() => (openAll = !openAll)}
        class="flex h-8 flex-none items-center rounded-md border border-(--mix-line) px-3 text-[13px] font-medium text-foreground transition-colors hover:border-primary aria-pressed:border-primary aria-pressed:bg-primary/15"
      >
        {openAll ? "Fold comparisons" : "Open all comparisons"}
      </button>
    </div>
  {/if}

  <ol class="flex list-none flex-col">
    {#each verses as ayah, i (i)}
      {@const n = i + 1}
      {#if stack === "a"}
        <!-- Option A: the live VerseRow — one row-wide padding, absolute toolbar, ruled extras. -->
        <li class="cur-row relative">
          <div class="absolute inset-x-5 top-[22px] flex items-center justify-between gap-2 sm:inset-x-9">
            <span class="font-mono text-[11px] tracking-wide text-muted-foreground">{surahNum}:{n}</span>
            <div class="flex items-center gap-0.5">
              {#each TOOLS as tool (tool.icon)}
                <button
                  type="button"
                  aria-label={tool.label}
                  class="flex size-[30px] items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground"
                >
                  <Icon name={tool.icon} size={15} />
                </button>
              {/each}
            </div>
          </div>
          <p dir="rtl" lang="ar" class="arabic-line">
            {ayah}<span class="ayah-ornament">&#x06DD;{toArabicDigits(n)}</span>
          </p>
          {#each translations as t (t.id)}
            <span class="cur-extra" dir={t.direction === "rtl" ? "rtl" : "auto"} lang={t.languageCode}>
              <span class="cur-label">{t.translator}</span>{t.texts[i]}
            </span>
          {/each}
        </li>
      {:else}
        <!-- Options B–E: each piece owns its padding (tools, Arabic, every translation). -->
        <li class={["vrow", tweaks.divider === "on" && "vrow--rule"]}>
          {#if tweaks.tools === "on"}
            <div class="vrow-tools">
              <span class="font-mono text-[11.5px] tracking-wide text-foreground-secondary">{surahNum}:{n}</span>
              <div class="flex items-center gap-0.5">
                {#each TOOLS as tool (tool.icon)}
                  <button
                    type="button"
                    aria-label={tool.label}
                    class="flex size-[30px] items-center justify-center rounded-md text-foreground-secondary transition-colors hover:text-foreground"
                  >
                    <Icon name={tool.icon} size={15} />
                  </button>
                {/each}
              </div>
            </div>
          {/if}
          <p dir="rtl" lang="ar" class="arabic-line vrow-arabic">
            {ayah}<span class="ayah-ornament">&#x06DD;{toArabicDigits(n)}</span>
          </p>

          {#if stack === "b"}
            <div class="vrow-trs">
              {#each translations as t (t.id)}
                {@render quietBlock(t, i)}
              {/each}
            </div>
          {:else if stack === "c"}
            <div class="vrow-trs">
              {#each translations as t, k (t.id)}
                {@render laneBlock(t, i, k)}
              {/each}
            </div>
          {:else if stack === "d"}
            <div class="vrow-trs lg:grid lg:gap-8" style:grid-template-columns={columns}>
              {#each translations as t (t.id)}
                <div dir={t.direction} lang={t.languageCode} class="tr-block min-w-0">
                  <span class="lg:sr-only">{@render label(t, undefined)}</span>
                  <p class={["tr-text", t.direction === "rtl" && "tr-rtl"]}>{t.texts[i]}</p>
                </div>
              {/each}
            </div>
          {:else if lead}
            <div class="vrow-trs">
              {@render quietBlock(lead, i)}
              {#if others.length > 0}
                <details class="compare" open={openAll}>
                  <summary
                    class="inline-flex cursor-pointer list-none items-center gap-1.5 rounded-md py-1 text-[13.5px] font-medium text-(--mix-accent-text) transition-colors hover:text-foreground"
                  >
                    <Icon name="plus" size={13} class="when-closed" />
                    <Icon name="minus" size={13} class="when-open" />
                    Compare with <span dir="auto">{otherNames}</span>
                  </summary>
                  <div class="vrow-trs pt-2">
                    {#each others as t (t.id)}
                      {@render quietBlock(t, i)}
                    {/each}
                  </div>
                </details>
              {/if}
            </div>
          {/if}
        </li>
      {/if}
    {/each}
  </ol>
{/if}

<style>
  .arabic-line {
    color: var(--quran-foreground);
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.15;
    text-align: start;
  }

  .reading-block {
    border-bottom: 1px solid var(--mix-line);
    padding: 2rem min(var(--row-x), 6vw);
  }

  .reading-flow {
    color: var(--quran-foreground);
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.35;
    text-align: justify;
    text-align-last: center;
    word-spacing: 0.14em;
  }

  .list-head {
    border-bottom: 1px solid var(--mix-line);
    padding: 0 min(var(--row-x), 6vw) 1rem;
  }

  .list-head--sticky {
    background: color-mix(in oklch, var(--mix-reader) 94%, transparent);
    backdrop-filter: blur(12px);
    border-top: 1px solid var(--mix-line);
    padding-bottom: 0.75rem;
    padding-top: 0.75rem;
    position: sticky;
    top: 61px;
    z-index: 20;
  }

  /* ── Option A: the live VerseRow stacking, copied verbatim. ── */
  .cur-row {
    border-bottom: 1px solid var(--mix-line);
    padding: 62px 1.25rem 22px;
  }

  .cur-row:last-child {
    border-bottom: 0;
  }

  @media (min-width: 640px) {
    .cur-row {
      padding-inline: 2.25rem;
    }
  }

  .cur-extra {
    border-top: 1px solid var(--mix-line);
    color: var(--translation-foreground);
    display: block;
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 1.0625rem);
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    text-align: start;
  }

  .cur-label {
    color: var(--muted);
    font-size: 0.8rem;
    margin-inline-end: 0.4rem;
  }

  /* ── Options B–E: no row-wide padding; every piece pads itself (mix tweak panel). ── */
  .vrow {
    padding-bottom: var(--row-bottom);
    padding-inline: min(var(--row-x), 6vw);
  }

  .vrow--rule {
    border-bottom: 1px solid var(--mix-line);
  }

  .vrow--rule:last-child {
    border-bottom: 0;
  }

  .vrow-tools {
    align-items: center;
    display: flex;
    gap: 0.5rem;
    justify-content: space-between;
    padding-top: var(--tools-top);
  }

  .vrow-arabic {
    padding-bottom: var(--ar-bottom);
    padding-top: var(--ar-top);
  }

  .vrow-trs {
    display: flex;
    flex-direction: column;
    gap: var(--tr-gap);
  }

  .tr-block {
    max-width: 72ch;
    padding-block: var(--tr-y);
  }

  /* An RTL block sits on the right edge of an LTR column. */
  .tr-block--rtl {
    margin-left: auto;
  }

  .tr-label {
    color: var(--foreground-secondary);
    display: block;
    font-family: var(--font-sans);
    font-size: var(--label-size);
    font-weight: 600;
    margin-bottom: 0.35rem;
  }

  .tr-label-lang {
    color: var(--foreground-secondary);
    font-weight: 500;
  }

  .tr-text {
    color: color-mix(in oklch, var(--foreground) 90%, var(--mix-reader));
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 1.0625rem);
    line-height: var(--tr-lead);
    text-wrap: pretty;
  }

  /* Urdu and other Arabic-script translations: Naskh, larger, looser — the Latin UI face has
     no Arabic glyphs, so the live reader falls back to a cramped system font. */
  .tr-rtl {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--reader-translation-size, 1.0625rem) * 1.2);
    line-height: calc(var(--tr-lead) + 0.3);
  }

  .lane {
    background: color-mix(in oklch, var(--lane) var(--lane-tint), transparent);
    border-end-end-radius: 6px;
    border-inline-start: var(--lane-edge) solid var(--lane);
    border-start-end-radius: 6px;
    padding-inline-end: calc(var(--lane-inset) * 0.75);
    padding-inline-start: var(--lane-inset);
  }

  /* The wrapper takes the lane's direction so the gap sits between name and text even for a
     Latin name in an Urdu lane; one label size per script. */
  .lane-name {
    font-family: var(--font-sans);
    font-size: var(--label-size);
    font-weight: 700;
    margin-inline-end: 0.6em;
  }

  .lane-name :global(.lane-name--arabic) {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--label-size) * 1.15);
  }

  .compare > summary::-webkit-details-marker {
    display: none;
  }

  .compare :global(.when-open) {
    display: none;
  }

  .compare[open] :global(.when-open) {
    display: inline;
  }

  .compare[open] :global(.when-closed) {
    display: none;
  }
</style>
