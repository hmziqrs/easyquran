<script lang="ts">
  import { page } from "$app/state";
  import {
    QuranScript,
    type QuranScript as QuranScriptValue,
    type TranslationDirection,
  } from "#lib/data/quran-types.js";
  import { parseTajweedSegments, tajweedRuleColor } from "#lib/quran/view/tajweed.js";
  import { parseTransliterationSegments } from "#lib/quran/text-marks.js";
  import { translationIdFromSegments } from "#lib/data/quran.js";
  import { TRANSLATION_BY_ID } from "#lib/data/translations.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import {
    reader_translation_empty_verse,
    reader_transliteration_legend,
  } from "#lib/i18n/m/reader.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import AyahOrnament from "./AyahOrnament.svelte";
  import IndoPakAyah from "./IndoPakAyah.svelte";

  let {
    text,
    n,
    vKey,
    isTranslation = false,
    translationLang,
    translationDirection,
    translationName,
    pending = false,
    script = QuranScript.Uthmani,
  }: {
    text: string;
    n: number;
    vKey: string;
    isTranslation?: boolean;
    translationLang?: string;
    /** Declared direction of a translation flowed off-route; route params still win. */
    translationDirection?: TranslationDirection;
    /** Catalogue name of the flowed translation (empty-verse credit off-route). */
    translationName?: string;
    pending?: boolean;
    script?: QuranScriptValue;
  } = $props();

  const segments = $derived(script === QuranScript.Tajweed ? parseTajweedSegments(text) : null);
  const revealed = $derived(page.url.hash === `#ayah-${vKey.replace(":", "-")}`);

  // TRX-11: the translation line's direction is the route translation's
  // declared direction (catalogue), never a dir="auto" guess; off-route
  // reading flow (no /t/** segments) takes the flowed source's declared
  // direction instead of guessing auto.
  const routeDirection = $derived.by(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (lang === undefined || translator === undefined) return undefined;
    return TRANSLATION_BY_ID.get(translationIdFromSegments(lang, translator))?.direction;
  });
  const translationDir = $derived(routeDirection ?? translationDirection ?? "auto");

  const empty = $derived(isTranslation && !pending && text.trim() === "");
  const translit = $derived(
    isTranslation && !pending && !empty && text.includes("<")
      ? parseTransliterationSegments(text)
      : null,
  );

  function emptyNote(): string {
    const lang = page.params.lang;
    const translator = page.params.translator;
    const name =
      lang !== undefined && translator !== undefined
        ? TRANSLATION_BY_ID.get(translationIdFromSegments(lang, translator))?.name
        : undefined;
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_translation_empty_verse(
      { translator: name ?? translationName ?? translationLang ?? lang ?? "" },
      { locale: getLocale() as UiLocale },
    );
  }

  function legend(): string {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar), so
    // getLocale() only ever returns a UiLocale at runtime.
    return reader_transliteration_legend(undefined, { locale: getLocale() as UiLocale });
  }
</script>

<span id="ayah-{vKey.replace(':', '-')}" data-verse-key={vKey} class:revealed-ayah={revealed}>
  {#if isTranslation}
    <span class="verse-text translation-text text-translation-foreground" lang={translationLang} dir={translationDir}>
      {#if pending}<span class="pending" aria-hidden="true"></span>
      {:else if empty}<span class="verse-missing">{emptyNote()}</span>
      {:else if translit}
        {#each translit as segment, index (index)}
          {#if segment.underline}<u class="translit-long">{segment.text}</u>
          {:else if segment.strong}<b class="translit-assimilated">{segment.text}</b>
          {:else}{segment.text}{/if}
        {/each}
      {:else}{text}{/if}<span class="ayah-marker" data-verse-anchor={vKey}>{n}</span>
      {#if translit}<span class="translit-legend">{legend()}</span>{/if}
    </span>
  {:else}
    <span class="verse-text arabic-text text-quran-foreground" class:indopak={script === QuranScript.IndoPak} dir="rtl" lang="ar">
      {#if segments}
        {#each segments as segment, index (index)}
          {#if segment.rule}<span style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span>{:else}{segment.text}{/if}
        {/each}
      {:else if script === QuranScript.IndoPak}
        <IndoPakAyah {text} {n} {vKey} />
      {:else}
        {text}
      {/if}{#if script !== QuranScript.IndoPak}<AyahOrnament {vKey} {n} />{/if}
    </span>
  {/if}
</span>

<style>
  .arabic-text {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.15;
  }

  .arabic-text.indopak {
    font-family: "IndoPak Reader Compat";
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

  /* Empty translation verse (TRX-11): an editorial note, not verse text. */
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
    display: block;
    font-family: "Onest Variable", "Onest", "Noto Naskh Arabic", system-ui, sans-serif;
    font-size: 12px;
    margin-top: 4px;
  }

  .translit-long {
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }

  .translit-assimilated {
    font-weight: 600;
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
