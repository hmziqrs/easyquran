<script lang="ts">
  import { getLocale } from "#lib/paraglide/runtime.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_commentary_label,
    reader_translation_empty_verse,
  } from "#lib/i18n/m/reader.js";
  import { isTafsirTranslation } from "#lib/quran/text-marks.js";
  import {
    parseTransliterationSegments,
    type TransliterationSegment,
  } from "#lib/quran/text-marks.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import type { StackedTranslation } from "#lib/data/quran-types.js";

  /**
   * The stacked-translation lanes under one verse row (user pick, /design/mix
   * stack=c): one accent edge per translation and its name inline, all in one
   * colour. Extracted from VerseRow so non-reader editorial surfaces (azkar)
   * render the exact same lanes without inheriting the row's reader-mode
   * coupling — visibility is the caller's concern.
   */
  let {
    stacked = [],
    stackedPending = [],
    stackedErrored = [],
    errorLabel = "",
    leadId = undefined,
  }: {
    stacked?: readonly StackedTranslation[];
    stackedPending?: readonly string[];
    stackedErrored?: readonly string[];
    errorLabel?: string;
    /** The route's own translation lane, always sorted first (reader routes only). */
    leadId?: string;
  } = $props();

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
  // loading rows jumping ahead of ready ones. activeIds: a paused stack renders no lanes.
  const order = $derived(stackedTranslations.activeIds);
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

  function verseEmptyNote(who: string): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_translation_empty_verse(
      { translator: who },
      { locale: getLocale() as UiLocale },
    );
  }

  function laneEmpty(t: StackedTranslation): boolean {
    return t.text.trim() === "";
  }

  function laneTranslit(t: StackedTranslation): readonly TransliterationSegment[] | null {
    if (laneEmpty(t) || !t.text.includes("<")) return null;
    return parseTransliterationSegments(t.text);
  }

  function commentaryWord(): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_commentary_label(undefined, { locale: getLocale() as UiLocale });
  }
</script>

<!-- Lanes: one accent edge per translation and its name inline, all in one colour —
     per-lane hues and grey text read as noise (user, 2026-10-02). -->
<div class="verse-lanes">
  {#each extraRows as row (extraRowKey(row))}
    {#if row.kind === "skeleton"}
      <div class="verse-extra verse-extra--skeleton" aria-hidden="true">
        <span class="verse-lane-skeleton"></span>
      </div>
    {:else if row.kind === "error"}
      <span class="verse-extra verse-extra--error">{errorLabel}</span>
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

<style>
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

  .verse-lane-skeleton {
    background: var(--background-subtle);
    border-radius: 0.25rem;
    display: block;
    height: 1.2rem;
    width: min(24rem, 70%);
  }
</style>
