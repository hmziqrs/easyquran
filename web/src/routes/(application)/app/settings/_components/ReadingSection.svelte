<script lang="ts">
  import { Card } from "$lib/components";
  import {
    ARABIC_FONTS,
    TRANSLATION_FAMILIES,
    arabicFontStack,
    type ArabicFontId,
    type TranslationFamily,
  } from "$lib/config/reader-fonts";
  import { loadArabicFont } from "$lib/fonts/arabic-fonts";
  import {
    ARABIC_FONT_MAX,
    ARABIC_FONT_MIN,
    TRANSLATION_FONT_MAX,
    TRANSLATION_FONT_MIN,
  } from "$lib/stores/reader-core.svelte";
  import { reader, type ReaderMode } from "$lib/stores/reader.svelte";
  import { cn } from "$lib/utils";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["reading"];
  } = $props();

  const SAMPLE_ARABIC = "بِسْمِ ٱللَّهِ ٱلرَّحْمَٰنِ ٱلرَّحِيمِ";

  const pill = "rounded-md border px-3 py-1 text-xs transition-colors duration-150";
  const pillOn = "border-accent bg-accent-soft text-fg";
  const pillOff = "border-line-2 text-fg-2 hover:text-fg";

  function pillClass(active: boolean): string {
    return cn(pill, active ? pillOn : pillOff);
  }

  function fontLabel(fontId: ArabicFontId): string {
    if (fontId === "amiri") return copy.fontNames.amiri;
    if (fontId === "scheherazade-new") return copy.fontNames.scheherazade;
    return copy.fontNames.notoNaskh;
  }

  const modes: ReaderMode[] = ["verse", "reading"];

  function modeLabel(mode: ReaderMode): string {
    return mode === "verse" ? copy.modeNames.verse : copy.modeNames.reading;
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
    "flex h-7 w-9 items-center justify-center rounded-md border border-line-2 text-xs text-fg-2 transition-colors hover:border-line hover:text-fg disabled:pointer-events-none disabled:opacity-40";
</script>

<Card id={id} tabindex={-1} class="scroll-mt-24">
  <h2 class="text-sm font-semibold text-fg">{heading}</h2>
  <p class="mt-1 text-xs text-fg-3">{copy.intro}</p>

  <div class="mt-5 grid gap-6">
    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.mode}</div>
      <div class="flex gap-1.5">
        {#each modes as mode (mode)}
          <button
            type="button"
            class={pillClass(reader.mode === mode)}
            aria-pressed={reader.mode === mode}
            onclick={() => reader.setMode(mode)}>{modeLabel(mode)}</button
          >
        {/each}
      </div>
    </div>

    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.arabicFont}</div>
      <div class="flex flex-wrap gap-1.5">
        {#each ARABIC_FONTS as font (font.id)}
          <button
            type="button"
            class={pillClass(reader.arabicFont === font.id)}
            aria-pressed={reader.arabicFont === font.id}
            style:font-family={arabicFontStack(font.id)}
            onclick={() => chooseArabicFont(font.id)}>{fontLabel(font.id)}</button
          >
        {/each}
      </div>
    </div>

    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.arabicSize}</div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          class={stepperButton}
          aria-label="{copy.arabicSize} −"
          disabled={reader.arabicSizePx === `${ARABIC_FONT_MIN}px`}
          onclick={() => reader.smaller()}>−</button
        >
        <span class="min-w-10 text-center text-xs tabular-nums text-fg-2"
          >{reader.arabicSizePx}</span
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

    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.translationFont}</div>
      <div class="flex gap-1.5">
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

    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.translationSize}</div>
      <div class="flex items-center gap-2">
        <button
          type="button"
          class={stepperButton}
          aria-label="{copy.translationSize} −"
          disabled={reader.translationSizePx === `${TRANSLATION_FONT_MIN}px`}
          onclick={() => reader.shrinkTranslation()}>−</button
        >
        <span class="min-w-10 text-center text-xs tabular-nums text-fg-2"
          >{reader.translationSizePx}</span
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

    <div>
      <div class="mb-1.5 text-xs text-fg-3">{copy.preview}</div>
      <div class="overflow-x-auto rounded-lg border border-line bg-bg-2 px-4 py-3">
        <p class="preview-arabic" dir="rtl" lang="ar">{SAMPLE_ARABIC}</p>
        <p class="preview-translation" dir="auto">{copy.intro}</p>
      </div>
    </div>
  </div>
</Card>

<style>
  .preview-arabic {
    font-family: var(--reader-arabic-family, var(--font-arabic));
    font-size: var(--reader-arabic-size, 33px);
    line-height: 2.15;
    margin: 0;
    text-align: right;
    white-space: nowrap;
  }

  .preview-translation {
    font-family: var(--reader-translation-family, var(--font-sans));
    font-size: var(--reader-translation-size, 17px);
    line-height: 1.85;
    margin: 0.5rem 0 0;
  }
</style>
