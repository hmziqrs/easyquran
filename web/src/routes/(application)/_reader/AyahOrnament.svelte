<script lang="ts">
  import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from "#lib/components/ui/tooltip/index.js";
  import { parseKey, toArabicDigits, toEasternDigits } from "#lib/data/quran.js";
  import type { CatalogEntry, Place, SajdaEntry } from "#lib/data/quran-types.js";
  import { peekQuranData } from "#lib/data/quran-data-client.js";
  import { positionForGlobal, type ReaderPositionState } from "#lib/data/mushaf-divisions.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { getSearchCopy } from "#lib/i18n/search-copy.js";
  import { positionLabel } from "./position-label";

  let {
    vKey,
    n,
    digits = "arabic",
  }: {
    vKey: string;
    n: number;
    /** "urdu": Extended Arabic-Indic digits in their Urdu forms, as printed in IndoPak mushafs. */
    digits?: "arabic" | "urdu";
  } = $props();

  const numerals = $derived(digits === "urdu" ? toEasternDigits(n) : toArabicDigits(n));

  const copy = getReaderUiCopy();
  const searchCopy = getSearchCopy();

  interface OrnamentMeta {
    entry: CatalogEntry;
    position: ReaderPositionState;
    sajda: SajdaEntry | undefined;
  }

  /** Resolved lazily — bits-ui only mounts tooltip content when it opens, so the
       peek never runs for ornaments the user never hovers. */
  function meta(): OrnamentMeta | null {
    const data = peekQuranData();
    if (!data) return null;
    const { num: surah, n: ayah } = parseKey(vKey);
    const entry = data.surahByNum(surah);
    const global = data.globalIndexOf(surah, ayah);
    if (!entry || !global) return null;
    return {
      entry,
      position: positionForGlobal(data, global),
      sajda: data.sajdaAt(surah, ayah),
    };
  }

  function placeLabel(place: Place): string {
    if (place === "medinan") return searchCopy.navPlaceMedinan;
    return searchCopy.navPlaceMeccan;
  }

  function sajdaLabel(kind: SajdaEntry["kind"]): string {
    if (kind === "obligatory") return searchCopy.navSajdaObligatory;
    return searchCopy.navSajdaRecommended;
  }
</script>

<!-- Own provider: reader surfaces wrap one, but tests (and any future host) render the
     ornament standalone — the context must not be an external requirement. -->
<TooltipProvider delayDuration={300}>
  <Tooltip>
  <TooltipTrigger>
    {#snippet child({ props })}
      <span
        {...props}
        tabindex="-1"
        class="ayah-ornament cursor-help"
        data-verse-anchor={vKey}
        lang={digits === "urdu" ? "ur" : undefined}
      >&#x06DD;{numerals}</span>
    {/snippet}
  </TooltipTrigger>
  <!-- Card, not the default dark chip: surah identity (English meaning always present),
       Arabic name with Arabic-digit reference, localized mushaf position, sajda flag. -->
  <TooltipContent
    side="top"
    arrowClasses="bg-surface"
    class="flex-col items-start gap-1.5 rounded-lg border border-border bg-surface px-4 py-3 text-start shadow-lg"
  >
    {#if meta()}
      {@const m = meta()}
      {#if m}
        <span class="flex flex-wrap items-baseline gap-x-2">
          <span class="text-[13.5px] font-semibold text-foreground">{m.entry.name}</span>
          <span class="font-mono text-[12px] tabular-nums text-muted">{vKey}</span>
          <span class="text-[12px] text-muted">{m.entry.meaning}</span>
        </span>
        <span class="font-arabic text-[15px] text-foreground" lang="ar" dir="rtl"
          >{m.entry.arabic} · {toArabicDigits(m.entry.num)}:{toArabicDigits(n)}</span
        >
        <span class="text-[11.5px] tabular-nums text-muted">
          {placeLabel(m.entry.place)} · {positionLabel(copy, m.position)}
        </span>
        {#if m.sajda}
          <span
            class="mt-0.5 rounded-sm bg-primary-soft px-2 py-1 text-[11.5px] font-medium text-foreground"
          >
            {sajdaLabel(m.sajda.kind)}
          </span>
        {/if}
      {:else}
        <span class="font-mono text-[12px] text-muted">{vKey}</span>
      {/if}
    {/if}
  </TooltipContent>
  </Tooltip>
</TooltipProvider>
