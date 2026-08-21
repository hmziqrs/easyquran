import { browser } from "$app/environment";

import {
  DEFAULT_ARABIC_FONT,
  DEFAULT_TRANSLATION_FAMILY,
  arabicFontStack,
  translationFamilyStack,
  type ArabicFontId,
  type TranslationFamily,
} from "$lib/config/reader-fonts";
import type { ReaderMode } from "./reader-core.svelte";

export function applyReaderPresentation(
  mode: ReaderMode,
  fontSize: number,
  arabicFont: ArabicFontId = DEFAULT_ARABIC_FONT,
  translationSize = 17,
  translationFamily: TranslationFamily = DEFAULT_TRANSLATION_FAMILY,
): void {
  if (!browser) return;
  const root = document.documentElement;
  root.dataset.readerMode = mode;
  root.style.setProperty("--reader-arabic-size", `${fontSize}px`);
  root.dataset.arabicFont = arabicFont;
  root.style.setProperty("--reader-arabic-family", arabicFontStack(arabicFont));
  root.style.setProperty("--reader-translation-size", `${translationSize}px`);
  root.style.setProperty("--reader-translation-family", translationFamilyStack(translationFamily));
}
