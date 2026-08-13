import {
  legal_eyebrow,
  legal_placeholder_note,
  legal_updated,
  legal_updated_date,
} from "$lib/i18n/m/legal";
import type { MarketingLocale, MarketingSeoCopy } from "$lib/i18n/marketing-copy";

export interface LegalSection {
  /** Stable across locales so the DOM key never depends on translated text. */
  id: string;
  heading: string;
  body: string;
}

export interface LegalPageCopy {
  seo: MarketingSeoCopy;
  eyebrow: string;
  heading: string;
  updated: string;
  placeholderNote: string;
  sections: LegalSection[];
  outroPrompt: string;
  outroLink: string;
}

/** Legal chrome shared by the privacy and terms pages. Four messages, so sharing costs nothing. */
export function resolveLegalChrome(locale: MarketingLocale) {
  return {
    eyebrow: legal_eyebrow(undefined, { locale }),
    updated: legal_updated({ date: legal_updated_date(undefined, { locale }) }, { locale }),
    placeholderNote: legal_placeholder_note(undefined, { locale }),
  };
}
