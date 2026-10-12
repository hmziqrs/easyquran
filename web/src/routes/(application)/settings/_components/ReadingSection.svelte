<script lang="ts">
  import { onMount } from "svelte";

  import {
    TRANSLATION_FAMILIES,
    arabicFontStack,
    enabledArabicFontIds,
    type ArabicFontId,
    type TranslationFamily,
  } from "#lib/config/reader-fonts.js";
  import { loadArabicFont } from "#lib/fonts/arabic-fonts.js";
  import {
    ARABIC_FONT_MAX,
    ARABIC_FONT_MIN,
    TRANSLATION_FONT_MAX,
    TRANSLATION_FONT_MIN,
  } from "#lib/stores/reader-core.svelte.js";
  import { QuranSourceId, type QuranSourceId as QuranSourceIdValue } from "#lib/data/quran-types.js";
  import { clientQuranEnvironment } from "#lib/quran/client-environment.js";
  import { isScriptEnabled } from "#lib/quran/script-availability.js";
  import { parseTajweedSegments, tajweedRuleColor } from "#lib/quran/view/tajweed.js";
  import { reader, type ReaderMode } from "#lib/stores/reader.svelte.js";
  import { prefs } from "#lib/stores/prefs.svelte.js";
  import { cn } from "#lib/utils.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["reading"];
  } = $props();

  // Preview mirrors the selected script: same two verses (94:5–6), verbatim text per
  // source DB. Tajweed renders parsed rule runs; IndoPak uses the reader compat face.
  interface SpecimenVerse {
    readonly marker: string;
    readonly text: string;
    readonly sample: () => string;
  }

  function specimenVerses(first: string, second: string): readonly SpecimenVerse[] {
    return [
      { marker: "٥", text: first, sample: () => copy.sampleAlt },
      { marker: "٦", text: second, sample: () => copy.sample },
    ];
  }

  const SPECIMEN = {
    [QuranSourceId.AnnotatedUthmani]: specimenVerses(
      "فَإِنَّ مَعَ ٱلْعُسْرِ يُسْرًا",
      "إِنَّ مَعَ ٱلْعُسْرِ يُسْرًۭا",
    ),
    [QuranSourceId.TanzilUthmani]: specimenVerses(
      "فَإِنَّ مَعَ ٱلْعُسْرِ يُسْرًا",
      "إِنَّ مَعَ ٱلْعُسْرِ يُسْرًا",
    ),
    [QuranSourceId.TanzilSimpleClean]: specimenVerses("فإن مع العسر يسرا", "إن مع العسر يسرا"),
    [QuranSourceId.Indopak]: specimenVerses(
      "فَاِنَّ مَعَ الۡعُسۡرِ يُسۡرًا ۙ",
      "اِنَّ مَعَ الۡعُسۡرِ يُسۡرًا ؕ",
    ),
    [QuranSourceId.Tajweed]: specimenVerses(
      "فَإِ[g[نّ]َ مَعَ [h:14550[ٱ]لْعُسْرِ يُسْرًا",
      "إِ[g[نّ]َ مَعَ [h:14550[ٱ]لْعُسْرِ يُسْرًا",
    ),
  } satisfies Record<QuranSourceIdValue, readonly SpecimenVerse[]>;

  const specimen = $derived(SPECIMEN[reader.arabicScript]);
  const specimenIsTajweed = $derived(reader.arabicScript === QuranSourceId.Tajweed);
  const specimenIsIndopak = $derived(reader.arabicScript === QuranSourceId.Indopak);

  onMount(() => {
    // Every selectable face loads up front so the pills and the specimen render their
    // real typefaces instead of fallbacks (FontFace loading is explicit, not CSS-driven).
    for (const fontId of enabledArabicFontIds(clientQuranEnvironment())) {
      void loadArabicFont(fontId);
    }
  });

  const pill =
    "flex items-center rounded-pill border px-3.5 py-2 text-start text-caption transition-colors duration-150";
  const pillOn = "border-primary bg-primary-soft text-foreground";
  const pillOff = "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";
  const pillDisabled = "cursor-not-allowed opacity-50";

  function pillClass(active: boolean): string {
    return cn(pill, active ? pillOn : pillOff);
  }

  const environment = clientQuranEnvironment();
  const arabicFonts = enabledArabicFontIds(environment);

  const SCRIPTS: readonly QuranSourceIdValue[] = [
    QuranSourceId.AnnotatedUthmani,
    QuranSourceId.TanzilUthmani,
    QuranSourceId.TanzilSimpleClean,
    QuranSourceId.Indopak,
    QuranSourceId.Tajweed,
  ];

  // Production-disabled scripts stay visible on purpose (campaign decision): the pills
  // render disabled with one note under the group, never hidden. See script-availability.
  const hasDisabledScripts = SCRIPTS.some((script) => !isScriptEnabled(script, environment));

  function scriptPillClass(script: QuranSourceIdValue, enabled: boolean): string {
    if (!enabled) return cn(pill, pillOff, pillDisabled);
    return pillClass(reader.arabicScript === script);
  }

  function scriptLabel(script: QuranSourceIdValue): string {
    if (script === QuranSourceId.AnnotatedUthmani) return copy.scriptNames.annotatedUthmani;
    if (script === QuranSourceId.TanzilSimpleClean) return copy.scriptNames.simpleClean;
    if (script === QuranSourceId.Indopak) return copy.scriptNames.indopak;
    if (script === QuranSourceId.Tajweed) return copy.scriptNames.tajweed;
    return copy.scriptNames.uthmani;
  }

  function fontLabel(fontId: ArabicFontId): string {
    if (fontId === "amiri") return copy.fontNames.amiri;
    if (fontId === "scheherazade-new") return copy.fontNames.scheherazade;
    if (fontId === "kfgqpc-hafs") return copy.fontNames.hafs;
    if (fontId === "kfgqpc-taha-v1") return copy.fontNames.tahaV1;
    if (fontId === "kfgqpc-taha-v1-bold") return copy.fontNames.tahaV1Bold;
    return copy.fontNames.notoNaskh;
  }

  const modes: ReaderMode[] = ["verse", "reading"];

  function modeLabel(mode: ReaderMode): string {
    return mode === "verse" ? copy.modeNames.verse : copy.modeNames.reading;
  }

  // Reading never asks which translation: Ayah-by-Ayah stacks every translation, and the
  // reader's Arabic / Translation toggle picks what Reading flows (stores/reading-text).
  function selectMode(mode: ReaderMode): void {
    reader.setMode(mode);
  }

  function chooseScript(script: QuranSourceIdValue): void {
    if (!isScriptEnabled(script, environment)) return;
    reader.setArabicScript(script);
  }

  function chooseArabicFont(fontId: ArabicFontId): void {
    reader.setArabicFont(fontId);
    void loadArabicFont(fontId);
  }

  const families: TranslationFamily[] = TRANSLATION_FAMILIES.map((f) => f.id);

  function familyLabel(family: TranslationFamily): string {
    return family === "sans" ? copy.fontFamilies.sans : copy.fontFamilies.serif;
  }

  const stepperButton =
    "flex h-9 w-10 items-center justify-center rounded-pill border border-border-strong text-caption text-foreground-secondary transition-colors hover:border-border hover:text-foreground disabled:pointer-events-none disabled:opacity-40";

  // Same toggle idiom as the privacy section (aria-pressed pill, on/off copy).
  const resumeToggle = "rounded-pill border px-3.5 py-2.5 text-caption transition-colors duration-150";
  const resumeToggleOn = "border-primary bg-primary-soft text-foreground";
  const resumeToggleOff =
    "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";

  function toggleInstantResume(): void {
    prefs.setInstantResume(!prefs.instantResume);
  }
</script>

<div id={id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-medium tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div class="mt-4 lg:grid lg:grid-cols-[minmax(0,1fr)_minmax(400px,480px)] lg:items-start lg:gap-6">
    <aside class="lg:sticky lg:top-[76px] lg:order-2" aria-label={copy.preview}>
      <div class="rounded-xl border border-border-strong bg-background-subtle px-5 py-4 sm:px-6 sm:py-5">
        <p class="eyebrow">{copy.preview}</p>
        {#if reader.mode === "verse"}
          <div class="mt-3.5 grid gap-5">
            {#each specimen as verse (verse.marker)}
              <div>
                <p class:indopak-spec={specimenIsIndopak} class="spec-arabic" dir="rtl" lang="ar"
                  >{#if specimenIsTajweed}{#each parseTajweedSegments(verse.text) as segment}{#if segment.rule}<span
                        class="tajweed-run"
                        style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span
                      >{:else}{segment.text}{/if}{/each}{:else}{verse.text}{/if}<span
                    class="ayah-marker">{verse.marker}</span></p
                >
                <p class="spec-translation" dir="auto">{verse.sample()}</p>
              </div>
            {/each}
          </div>
        {:else}
          <p class:indopak-spec={specimenIsIndopak} class="spec-flow mt-3.5" dir="rtl" lang="ar">
            {#each specimen as verse (verse.marker)}{#if specimenIsTajweed}{#each parseTajweedSegments(
                verse.text,
              ) as segment}{#if segment.rule}<span
                  class="tajweed-run"
                  style:color={tajweedRuleColor(segment.rule)}>{segment.text}</span
                >{:else}{segment.text}{/if}{/each}{:else}{verse.text}{/if}<span class="ayah-marker"
                >{verse.marker}</span
              >{" "}
            {/each}
          </p>
        {/if}
      </div>
    </aside>

    <div
      class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface lg:order-1 lg:mt-0"
    >
      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.mode}</span>
        <div class="mt-2.5 grid max-w-md gap-1.5">
          {#each modes as mode (mode)}
            <button
              type="button"
              class={pillClass(reader.mode === mode)}
              aria-pressed={reader.mode === mode}
              onclick={() => selectMode(mode)}>{modeLabel(mode)}</button
            >
          {/each}
        </div>
      </div>

      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.arabicScript}</span>
        <div class="mt-2.5 grid max-w-md gap-1.5">
          {#each SCRIPTS as script (script)}
            {@const scriptEnabled = isScriptEnabled(script, environment)}
            <button
              type="button"
              class={scriptPillClass(script, scriptEnabled)}
              aria-pressed={reader.arabicScript === script}
              disabled={!scriptEnabled}
              onclick={() => chooseScript(script)}>{scriptLabel(script)}</button
            >
          {/each}
        </div>
        {#if hasDisabledScripts}
          <p class="mt-2 max-w-md text-caption leading-snug text-muted">{copy.scriptDisabledNote}</p>
        {/if}
      </div>

      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.arabicFont}</span>
        <div class="mt-2.5 grid max-w-md gap-1.5">
          {#each arabicFonts as fontId (fontId)}
            <button
              type="button"
              class={pillClass(reader.arabicFont === fontId)}
              aria-pressed={reader.arabicFont === fontId}
              style:font-family={arabicFontStack(fontId)}
              onclick={() => chooseArabicFont(fontId)}>{fontLabel(fontId)}</button
            >
          {/each}
        </div>
      </div>

      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.arabicSize}</span>
        <div class="mt-2.5 flex items-center gap-2.5" role="group" aria-label={copy.arabicSize}>
          <button
            type="button"
            class={stepperButton}
            aria-label="{copy.arabicSize} −"
            disabled={reader.arabicSizePx === `${ARABIC_FONT_MIN}px`}
            onclick={() => reader.smaller()}>−</button
          >
          <span
            class="min-w-12 text-center text-caption tabular-nums text-foreground-secondary"
            aria-live="polite">{reader.arabicSizePx}</span
          >
          <button
            type="button"
            class={stepperButton}
            aria-label="{copy.arabicSize} +"
            disabled={reader.arabicSizePx === `${ARABIC_FONT_MAX}px`}
            onclick={() => reader.bigger()}>+</button
          >
        </div>
      </div>

      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.translationFont}</span>
        <div class="mt-2.5 grid max-w-md gap-1.5">
          {#each families as family (family)}
            <button
              type="button"
              class={pillClass(reader.translationFamily === family)}
              aria-pressed={reader.translationFamily === family}
              onclick={() => reader.setTranslationFamily(family)}>{familyLabel(family)}</button
            >
          {/each}
        </div>
      </div>

      <div class="px-4 py-3.5 sm:px-5">
        <span class="text-[14.5px] font-medium text-foreground">{copy.translationSize}</span>
        <div
          class="mt-2.5 flex items-center gap-2.5"
          role="group"
          aria-label={copy.translationSize}
        >
          <button
            type="button"
            class={stepperButton}
            aria-label="{copy.translationSize} −"
            disabled={reader.translationSizePx === `${TRANSLATION_FONT_MIN}px`}
            onclick={() => reader.shrinkTranslation()}>−</button
          >
          <span
            class="min-w-12 text-center text-caption tabular-nums text-foreground-secondary"
            aria-live="polite">{reader.translationSizePx}</span
          >
          <button
            type="button"
            class={stepperButton}
            aria-label="{copy.translationSize} +"
            disabled={reader.translationSizePx === `${TRANSLATION_FONT_MAX}px`}
            onclick={() => reader.growTranslation()}>+</button
          >
        </div>
      </div>

      <!-- Feature 5.9 (HOME-01 second pass): the dormant prefs.instantResume pref gets its UI here. -->
      <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <div class="min-w-0">
          <span class="text-[14.5px] font-medium text-foreground">{copy.resume}</span>
          <p class="mt-0.5 text-caption leading-snug text-muted">{copy.resumeNote}</p>
        </div>
        <button
          type="button"
          aria-pressed={prefs.instantResume}
          onclick={toggleInstantResume}
          class={cn(
            resumeToggle,
            "shrink-0",
            prefs.instantResume ? resumeToggleOn : resumeToggleOff,
          )}
        >
          {prefs.instantResume ? copy.on : copy.off}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .spec-arabic {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.15;
    margin: 0;
    text-align: right;
  }

  .spec-translation {
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 17px);
    line-height: 1.85;
    margin: 0.5rem 0 0;
    color: var(--fg-2);
  }

  .spec-flow {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.35;
    margin: 0;
    text-align: justify;
    text-align-last: center;
    word-spacing: 0.14em;
  }

  /* Same compat face the reader's IndoPak path uses (IndoPakAyah.svelte); declared here
     so the settings specimen renders IndoPak text without mounting the reader. */
  @font-face {
    font-family: "IndoPak Reader Compat";
    src:
      url("/fonts/indopak-reader-compat-v4.woff2") format("woff2"),
      url("/fonts/indopak-reader-compat-v4.ttf") format("truetype");
    font-display: swap;
  }

  .indopak-spec {
    font-family: "IndoPak Reader Compat", var(--reader-arabic-family, var(--font-arabic));
  }

  .tajweed-run {
    /* Color comes from the inline style (rule palette in lib/quran/view/tajweed.ts). */
    text-decoration: none;
  }
</style>
