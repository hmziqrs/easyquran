<script lang="ts">
  import { onMount, type Component } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import { page } from "$app/state";
  import { reader } from "#lib/stores/reader.svelte.js";
  import { QuranScript, type QuranScript as QuranScriptValue } from "#lib/data/quran-types.js";
  import { parseTajweedSegments, tajweedRuleColor } from "#lib/quran/view/tajweed.js";
  import {
    isTafsirTranslation,
    parseTransliterationSegments,
    stripTransliterationMarkup,
    type TransliterationSegment,
  } from "#lib/quran/text-marks.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { translationIdFromSegments } from "#lib/data/quran.js";
  import { TRANSLATION_BY_ID } from "#lib/data/translations.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import {
    reader_commentary_label,
    reader_translation_empty_verse,
    reader_transliteration_legend,
  } from "#lib/i18n/m/reader.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import type { StackedTranslation, TranslationDirection } from "#lib/data/quran-types.js";
  import AyahOrnament from "./AyahOrnament.svelte";
  import IndoPakAyah from "./IndoPakAyah.svelte";

  let {
    text,
    n,
    vKey,
    isTranslation,
    translationLang,
    translationDirection,
    translationName,
    pending = false,
    arabicPending = false,
    leadId,
    script = QuranScript.Uthmani,
    stacked = [],
    stackedPending = [],
    stackedErrored = [],
    stackedErrorLabel = "",
    localPage,
    virtualIndex,
    totalAyahs,
    virtualGap = 0,
    measure,
  }: {
    text: string;
    n: number;
    vKey: string;
    isTranslation?: boolean;
    /** Language of a translation flowed in Reading on a route of another source. */
    translationLang?: string;
    /** Declared direction of the flowed translation (off-route); route params still win. */
    translationDirection?: TranslationDirection;
    /** Catalogue name of the flowed translation (empty-verse credit off-route). */
    translationName?: string;
    /** Reading flows a stacked translation whose text has not arrived yet. */
    pending?: boolean;
    /** A translation route's Arabic has not arrived yet (Ayah-by-Ayah keeps a line for it). */
    arabicPending?: boolean;
    /** The translation route's own lane, always first. */
    leadId?: string;
    script?: QuranScriptValue;
    stacked?: readonly StackedTranslation[];
    stackedPending?: readonly string[];
    stackedErrored?: readonly string[];
    stackedErrorLabel?: string;
    localPage?: number;
    virtualIndex?: number;
    totalAyahs?: number;
    virtualGap?: number;
    measure?: Attachment<HTMLElement>;
  } = $props();
  let Tools = $state<Component<{ text: string; vKey: string }> | null>(null);

  const ayahId = $derived(`ayah-${vKey.replace(":", "-")}`);
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
  function rowSourceId(row: ExtraRow): string {
    return row.kind === "text" ? row.t.sourceId : row.sourceId;
  }
  // Lanes keep the reader's stacked order (and so their hue) while sources load, instead of
  // loading rows jumping ahead of ready ones.
  const order = $derived(stackedTranslations.ids);
  function rank(id: string): number {
    if (id === leadId) return -1;
    const i = order.indexOf(id);
    return i === -1 ? order.length : i;
  }
  const extraRows = $derived<ExtraRow[]>(
    [
      ...stackedPending.map((sourceId): ExtraRow => ({ kind: "skeleton", sourceId })),
      ...stackedErrored.map((sourceId): ExtraRow => ({ kind: "error", sourceId })),
      ...stacked.map((t): ExtraRow => ({ kind: "text", t })),
    ].sort((a, b) => rank(rowSourceId(a)) - rank(rowSourceId(b))),
  );

  /**
   * quran.com-style credit after the text: the translator, then the work in parentheses when
   * it is named differently ("Dr. Israr Ahmad (Bayan-ul-Quran)"); one name when they match.
   */
  function laneCredit(t: StackedTranslation): string {
    const who = t.translator ?? t.name ?? t.language;
    const work = t.name;
    if (!work || work.trim().toLowerCase() === who.trim().toLowerCase()) return who;
    return `${who} (${work})`;
  }

  // The route's own translation entry, when the reader sits on /t/** segments
  // (TRX-11): direction and name come from the catalogue, never guessed.
  const routeTranslationEntry = $derived.by(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (lang === undefined || translator === undefined) return undefined;
    return TRANSLATION_BY_ID.get(translationIdFromSegments(lang, translator));
  });

  /** Declared direction for the flowed translation line; `auto` only when neither
   * the route nor the flowed source declares one. */
  const flowDirection = $derived(routeTranslationEntry?.direction ?? translationDirection ?? "auto");

  const mainEmpty = $derived(translationActive && !pending && text.trim() === "");
  const mainTranslit = $derived(
    translationActive && !pending && !mainEmpty && text.includes("<")
      ? parseTransliterationSegments(text)
      : null,
  );

  function verseEmptyNote(who: string): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_translation_empty_verse(
      { translator: who },
      { locale: getLocale() as UiLocale },
    );
  }

  /** Name shown for an empty verse: the route's catalogue name, the flowed
   * source's name, else its language. */
  function mainEmptyWho(): string {
    return routeTranslationEntry?.name ?? translationName ?? translationLang ?? page.params.lang ?? "";
  }

  function laneEmpty(t: StackedTranslation): boolean {
    return t.text.trim() === "";
  }

  function laneTranslit(t: StackedTranslation): readonly TransliterationSegment[] | null {
    if (laneEmpty(t) || !t.text.includes("<")) return null;
    return parseTransliterationSegments(t.text);
  }

  // Legend shows once per row, only when a rendered line carries transliteration marks.
  const showsTranslitLegend = $derived(
    mainTranslit !== null || extraRows.some((row) => row.kind === "text" && row.t.text.includes("<")),
  );

  function commentaryWord(): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_commentary_label(undefined, { locale: getLocale() as UiLocale });
  }

  function translitLegend(): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_transliteration_legend(undefined, { locale: getLocale() as UiLocale });
  }

  onMount(() => {
    void import("./VerseTools.svelte")
      .then((module) => {
        Tools = module.default;
      })
      .catch(() => {});
  });
</script>

<li
  {@attach measure}
  id={ayahId}
  data-verse-key={vKey}
  data-local-page={localPage}
  data-index={virtualIndex}
  aria-posinset={totalAyahs === undefined ? undefined : n}
  aria-setsize={totalAyahs}
  style:margin-block-start={`${virtualGap}px`}
  class="verse-row group relative scroll-mt-24 border-b border-reader-divider pb-[26px] pt-[60px] {isRevealed
    ? 'revealed-ayah'
    : ''} {reader.isReadingMode && translationActive
    ? 'verse-row--translation-flow'
    : ''}"
>
  {#if translationActive}
    <!-- TRX-11: dir comes from the translation's declared direction (catalogue),
         never dir="auto" — an empty verse in an RTL translation otherwise flips
         its marker to the wrong side. -->
    <span
      lang={translationLang ?? page.params.lang}
      dir={flowDirection}
      class="verse-text verse-text--translation leading-[1.85] text-translation-foreground"
    >
      {#if pending}<span class="flow-pending" aria-hidden="true"></span>
      {:else if mainEmpty}
        <span class="verse-missing">{verseEmptyNote(mainEmptyWho())}</span>
      {:else if mainTranslit}
        {#each mainTranslit as segment, index (index)}
          {#if segment.underline}<u class="translit-long">{segment.text}</u>
          {:else if segment.strong}<b class="translit-assimilated">{segment.text}</b>
          {:else}{segment.text}{/if}
        {/each}
      {:else}{text}{/if}<span
        class="ayah-marker translation-marker"
        data-verse-anchor={vKey}>{n}</span
      >
    </span>
  {:else}
    <span
      dir="rtl"
      lang="ar"
      class="verse-text font-arabic leading-[2.15] text-quran-foreground"
      class:indopak={script === QuranScript.IndoPak}
      style="font-size:var(--reader-arabic-size, 33px)"
    >
      {#if arabicPending}<span class="arabic-pending" aria-hidden="true"></span>{:else if tajweedSegments}
        {#each tajweedSegments as segment, index (segmentKey(segment, index))}
          {#if segment.rule}
            <span class="tajweed-run" style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span
            >
          {:else}
            {segment.text}
          {/if}
        {/each}
      {:else if script === QuranScript.IndoPak}
        <IndoPakAyah {text} {n} {vKey} />
      {:else}
        {text}
      {/if}{#if script !== QuranScript.IndoPak || arabicPending}<AyahOrnament {vKey} {n} />{/if}
    </span>
  {/if}

  {#if reader.isVerseMode && extraRows.length > 0}
    <!-- Lanes (user pick, /design/mix stack=c): one accent edge per translation and its name
         inline, all in one colour — per-lane hues and grey text read as noise (user, 2026-10-02). -->
    <div class="verse-lanes">
      {#each extraRows as row (extraRowKey(row))}
        {#if row.kind === "skeleton"}
          <div class="verse-extra verse-extra--skeleton" aria-hidden="true">
            <span class="verse-lane-skeleton"></span>
          </div>
        {:else if row.kind === "error"}
          <span class="verse-extra verse-extra--error">{stackedErrorLabel}</span>
        {:else}
          {@const tafsir = isTafsirTranslation(row.t.sourceId)}
          {@const translit = laneTranslit(row.t)}
          <!-- dir is the lane's declared direction (TRX-11), never auto. Tafsir
               lanes render as commentary (TRX-01): a distinct muted block whose
               credit line carries the word "commentary" in the UI font. -->
          <span
            class="verse-extra"
            class:verse-extra--tafsir={tafsir}
            dir={row.t.direction}
            lang={row.t.languageCode}
          >
            {#if laneEmpty(row.t)}
              <span class="verse-missing">{verseEmptyNote(row.t.name ?? row.t.language)}</span>
            {:else if translit}
              {#each translit as segment, index (index)}
                {#if segment.underline}<u class="translit-long">{segment.text}</u>
                {:else if segment.strong}<b class="translit-assimilated">{segment.text}</b>
                {:else}{segment.text}{/if}
              {/each}
            {:else}{row.t.text}{/if}
            <!-- The credit takes the lane's direction (the dash leads on the lane's own side);
                 the bdi keeps a Latin name readable inside an Urdu lane. -->
            <span class="verse-extra-credit" dir={row.t.direction}>— <bdi>{laneCredit(row.t)}{#if tafsir} ({commentaryWord()}){/if}</bdi></span>
          </span>
        {/if}
      {/each}
    </div>
  {/if}

  {#if showsTranslitLegend}
    <!-- TRX-02: the underline marks only make sense with the legend. -->
    <p class="translit-legend">{translitLegend()}</p>
  {/if}

  {#if Tools}
    <Tools text={stripTransliterationMarkup(text)} {vKey} />
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

  .verse-row .verse-text {
    font-family: var(--reader-arabic-family, var(--font-arabic));
  }

  .verse-row .verse-text.indopak {
    font-family: "IndoPak Reader Compat";
  }

  .verse-row .verse-text--translation {
    font-family: var(--reader-translation-family, var(--font-sans));
    text-align: start;
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

  :global([data-reader-mode="reading"] [data-source-kind="arabic"]) .verse-text {
    display: inline;
  }

  /* U10 continuous reading flow for translations: verses join one flowing,
     justified column; the existing pill ayah-marker stays inline as the
     subtle ayah boundary (echoes the Arabic ornament flow). Verse mode and
     the Arabic reading flow above are untouched. */
  :global([data-reader-mode="reading"] [data-source-kind="translation"]) .verse-row {
    display: inline;
    padding: 0;
    border: 0;
    background: transparent;
  }

  :global([data-reader-mode="reading"] [data-source-kind="translation"]) .verse-text {
    display: inline;
  }

  :global([data-reader-mode="reading"] [data-source-kind="translation"]) .translation-marker {
    margin-inline-end: 0.55em;
  }

  .verse-lanes {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 12px;
  }

  .verse-extra {
    border-end-end-radius: 6px;
    border-inline-start: 2px solid var(--primary);
    border-start-end-radius: 6px;
    color: var(--foreground);
    display: block;
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 1.0625rem);
    line-height: 1.75;
    max-width: 72ch;
    padding-inline: 14px 10px;
    text-align: start;
    text-wrap: pretty;
  }

  /* Urdu and other Arabic-script translations: Naskh, larger, looser, on the right edge.
     The Latin UI face has no Arabic glyphs, so they used to fall back to a cramped system font. */
  .verse-extra:dir(rtl) {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--reader-translation-size, 1.0625rem) * 1.2);
    line-height: 2.05;
    margin-left: auto;
  }

  /* Credit after the text, grey and small (user pick, quran.com's pattern): it reads as
     "this is a translation, by this translator", not as part of the text. */
  .verse-extra-credit {
    color: var(--foreground-secondary);
    display: block;
    font-family: "Onest Variable", "Onest", "Noto Naskh Arabic", system-ui, sans-serif;
    font-size: 13px;
    font-weight: 400;
    line-height: 1.5;
    margin-top: 4px;
  }

  .verse-extra--error {
    color: var(--muted);
    font-size: 0.95rem;
  }

  /* Commentary (tafsir) lane (TRX-01): visually distinct from translation
     lanes — a soft surface with a neutral edge instead of the accent edge — so
     it never reads as a second translation. The Arabic text keeps the Naskh
     face from .verse-extra:dir(rtl); the credit line (UI font) carries the
     word "commentary". */
  .verse-extra--tafsir {
    background: var(--background-subtle);
    border-inline-start: 2px solid var(--foreground-secondary);
  }

  /* Empty translation verse (TRX-11): the placeholder reads as an editorial
     note, not as verse text. */
  .verse-missing {
    color: var(--muted);
    display: inline-block;
    font-family: "Onest Variable", "Onest", "Noto Naskh Arabic", system-ui, sans-serif;
    font-size: 0.9em;
    font-style: italic;
    padding-block: 0.25rem;
  }

  .translit-legend {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
  }

  .translit-long {
    /* Long vowel: stretch it (legend explains the underline). */
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }

  .translit-assimilated {
    /* Silent/assimilated letter (TRX-02). */
    font-weight: 600;
  }

  .verse-lane-skeleton {
    background: var(--background-subtle);
    border-radius: 0.25rem;
    display: block;
    height: 1.2rem;
    width: min(24rem, 70%);
  }

  .verse-text--translation {
    font-size: var(--reader-translation-size, 1.0625rem);
  }

  .verse-text--translation:dir(rtl) {
    font-family: "Noto Naskh Arabic", var(--font-arabic-ui);
    font-size: calc(var(--reader-translation-size, 1.0625rem) * 1.2);
    line-height: 2.05;
  }

  .arabic-pending {
    background: var(--background-subtle);
    border-radius: 0.375rem;
    display: inline-block;
    height: 1.1em;
    vertical-align: middle;
    width: min(26rem, 70%);
  }

  .flow-pending {
    background: var(--background-subtle);
    border-radius: 0.25rem;
    display: inline-block;
    height: 0.9em;
    vertical-align: middle;
    width: min(18rem, 60%);
  }

</style>
