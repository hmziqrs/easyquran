import type { MarketingLocale, MarketingSeoCopy } from "$lib/i18n/marketing-copy";
import {
  faq_account_answer,
  faq_account_question,
  faq_backup_answer,
  faq_backup_question,
  faq_eyebrow,
  faq_free_answer,
  faq_free_question,
  faq_heading,
  faq_missing_link,
  faq_missing_prompt,
  faq_next_answer,
  faq_next_question,
  faq_offline_answer,
  faq_offline_question,
  faq_script_answer,
  faq_script_question,
  faq_seo_description,
  faq_seo_title,
  faq_translations_answer,
  faq_translations_question,
} from "$lib/i18n/m/faq";

export interface FaqEntry {
  /** Stable across locales: DOM keys and structured-data order must not depend on translation. */
  id: string;
  q: string;
  a: string;
}

export interface FaqResolvedCopy {
  seo: MarketingSeoCopy;
  eyebrow: string;
  heading: string;
  entries: FaqEntry[];
  missingPrompt: string;
  missingLink: string;
}

/** FAQ page copy. Also feeds the FAQPage structured data, so it is localized with the page. */
export function resolveFaqCopy(locale: MarketingLocale): FaqResolvedCopy {
  return {
    seo: {
      title: faq_seo_title(undefined, { locale }),
      description: faq_seo_description(undefined, { locale }),
      imageAlt: faq_seo_title(undefined, { locale }),
    },
    eyebrow: faq_eyebrow(undefined, { locale }),
    heading: faq_heading(undefined, { locale }),
    entries: [
      {
        id: "free",
        q: faq_free_question(undefined, { locale }),
        a: faq_free_answer(undefined, { locale }),
      },
      {
        id: "account",
        q: faq_account_question(undefined, { locale }),
        a: faq_account_answer(undefined, { locale }),
      },
      {
        id: "script",
        q: faq_script_question(undefined, { locale }),
        a: faq_script_answer(undefined, { locale }),
      },
      {
        id: "offline",
        q: faq_offline_question(undefined, { locale }),
        a: faq_offline_answer(undefined, { locale }),
      },
      {
        id: "next",
        q: faq_next_question(undefined, { locale }),
        a: faq_next_answer(undefined, { locale }),
      },
      {
        id: "translations",
        q: faq_translations_question(undefined, { locale }),
        a: faq_translations_answer(undefined, { locale }),
      },
      {
        id: "backup",
        q: faq_backup_question(undefined, { locale }),
        a: faq_backup_answer(undefined, { locale }),
      },
    ],
    missingPrompt: faq_missing_prompt(undefined, { locale }),
    missingLink: faq_missing_link(undefined, { locale }),
  };
}
