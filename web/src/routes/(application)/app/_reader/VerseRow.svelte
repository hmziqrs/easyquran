<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { page } from "$app/state";
  import { reader } from "$lib/stores/reader.svelte";
  import { toArabicDigits } from "$lib/data/quran";
  import { QuranScript, type QuranScript as QuranScriptValue } from "$lib/data/quran-types";
  import { parseTajweedSegments, tajweedRuleColor } from "$lib/quran/view/tajweed";
  import type { StackedTranslation } from "$lib/data/quran-types";

  let {
    text,
    n,
    vKey,
    isTranslation,
    script = QuranScript.Uthmani,
    onToggleNote,
    stacked = [],
    stackedPending = [],
    stackedErrored = [],
    stackedErrorLabel = "",
  }: {
    text: string;
    n: number;
    vKey: string;
    isTranslation?: boolean;
    script?: QuranScriptValue;
    onToggleNote?: () => void;
    stacked?: readonly StackedTranslation[];
    stackedPending?: readonly string[];
    stackedErrored?: readonly string[];
    stackedErrorLabel?: string;
  } = $props();
  let Tools = $state<
    Component<{ text: string; vKey: string; onToggleNote?: () => void }> | null
  >(null);

  const ayahId = $derived(`ayah-${vKey.replace(":", "-")}`);
  // Tajweed markup renders as colored runs (view-only; the stored/wire text is
  // verbatim — see lib/quran/view/tajweed.ts). Every other script renders raw.
  const tajweedSegments = $derived.by(() =>
    script === QuranScript.Tajweed ? parseTajweedSegments(text) : null,
  );
  function segmentKey(segment: { text: string; rule: string | null }, index: number): string {
    return `${index}:${segment.rule ?? "p"}`;
  }
  const isRevealed = $derived(page.url.hash === `#${ayahId}`);
  const translationActive = $derived(
    isTranslation ?? ("lang" in page.params && "translator" in page.params),
  );

  type ExtraRow =
    | { kind: "skeleton"; sourceId: string }
    | { kind: "error"; sourceId: string }
    | { kind: "text"; t: StackedTranslation };
  function extraRowKey(row: ExtraRow): string {
    if (row.kind === "text") return `text-${row.t.sourceId}`;
    return `${row.kind}-${row.sourceId}`;
  }
  const extraRows = $derived<ExtraRow[]>([
    ...stackedPending.map((sourceId): ExtraRow => ({ kind: "skeleton", sourceId })),
    ...stackedErrored.map((sourceId): ExtraRow => ({ kind: "error", sourceId })),
    ...stacked.map((t): ExtraRow => ({ kind: "text", t })),
  ]);

  onMount(() => {
    void import("./VerseTools.svelte")
      .then((module) => {
        Tools = module.default;
      })
      .catch(() => {});
  });
</script>

<li
  id={ayahId}
  data-verse-key={vKey}
  class="verse-row group relative scroll-mt-24 border-b border-reader-divider px-5 pb-[22px] pt-[62px] transition-colors sm:px-9 {isRevealed
    ? 'revealed-ayah'
    : ''}"
>
  {#if translationActive}
    <span
      lang={page.params.lang}
      dir="auto"
      class="verse-text verse-text--translation leading-[1.85] text-translation-foreground"
      style="font-size:var(--reader-translation-size, 1.0625rem)"
    >
      {text}<span class="ayah-marker translation-marker" data-verse-anchor={vKey}>{n}</span>
    </span>
  {:else}
    <span
      dir="rtl"
      lang="ar"
      class="verse-text font-arabic leading-[2.15] text-quran-foreground"
      style="font-size:var(--reader-arabic-size, 33px)"
    >
      {#if tajweedSegments}
        {#each tajweedSegments as segment, index (segmentKey(segment, index))}
          {#if segment.rule}
            <span class="tajweed-run" style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span
            >
          {:else}
            {segment.text}
          {/if}
        {/each}
      {:else}
        {text}
      {/if}<span class="ayah-ornament arabic-marker" data-verse-anchor={vKey}
        >&#x06DD;{toArabicDigits(n)}</span
      >
    </span>
  {/if}

  {#if reader.isVerseMode}
    {#each extraRows as row (extraRowKey(row))}
      {#if row.kind === "skeleton"}
        <div class="verse-extra verse-extra--skeleton" aria-hidden="true"></div>
      {:else if row.kind === "error"}
        <span class="verse-extra verse-extra--error">{stackedErrorLabel}</span>
      {:else}
        <span
          class="verse-extra"
          dir={row.t.direction === "rtl" ? "rtl" : "auto"}
          lang={row.t.languageCode}
          style="font-size:var(--reader-translation-size, 1.0625rem)"
        >
          <span class="verse-extra-label">{row.t.translator ?? row.t.language}</span>{row.t.text}
        </span>
      {/if}
    {/each}
  {/if}

  {#if Tools}
    <Tools {text} {vKey} {onToggleNote} />
  {/if}
</li>

<style>
  .verse-text {
    display: block;
    text-align: start;
  }

  .tajweed-run {
    /* Color comes from the inline style (rule palette in lib/quran/view/tajweed.ts). */
    text-decoration: none;
  }

  /* §21 ayah hover: quiet surface tint (transparent at rest); suppressed in continuous reading mode. */
  .verse-row:hover {
    background: var(--surface-hover);
  }

  .verse-row .verse-text {
    font-family: var(--reader-arabic-family, var(--font-arabic));
  }

  .verse-row .verse-text--translation {
    font-family: var(--reader-translation-family, var(--font-sans));
    text-align: start;
  }

  .verse-row .arabic-marker {
    font-family: var(--reader-arabic-family, var(--font-arabic));
  }

  .verse-row .translation-marker {
    font-family: var(--reader-translation-family, var(--font-sans));
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-row {
    display: inline;
    padding: 0;
    border: 0;
    background: transparent;
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-row:hover {
    background: transparent;
  }

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-text {
    display: inline;
  }

  .verse-extra {
    display: block;
    border-top: 1px solid var(--reader-divider);
    color: var(--translation-foreground);
    font-family: var(--reader-translation-family, var(--font-sans));
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    text-align: start;
  }

  .verse-extra-label {
    color: var(--muted);
    font-size: 0.8rem;
    margin-inline-end: 0.4rem;
  }

  .verse-extra--error {
    color: var(--muted);
    font-size: 0.95rem;
  }

  .verse-extra--skeleton {
    background: var(--background-subtle);
    border-radius: 0.25rem;
    height: 1.2rem;
  }
</style>
