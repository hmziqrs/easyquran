import type { LegalPageCopy } from "$lib/i18n/legal-copy";
import { resolveLegalChrome } from "$lib/i18n/legal-copy";
import {
  legal_privacy_accounts_body,
  legal_privacy_accounts_heading,
  legal_privacy_changes_body,
  legal_privacy_changes_heading,
  legal_privacy_children_body,
  legal_privacy_children_heading,
  legal_privacy_choices_body,
  legal_privacy_choices_heading,
  legal_privacy_collect_body,
  legal_privacy_collect_heading,
  legal_privacy_device_body,
  legal_privacy_device_heading,
  legal_privacy_heading,
  legal_privacy_hosting_body,
  legal_privacy_hosting_heading,
  legal_privacy_notifications_body,
  legal_privacy_notifications_heading,
  legal_privacy_questions_link,
  legal_privacy_questions_prompt,
  legal_privacy_seo_description,
  legal_privacy_seo_title,
  legal_privacy_summary,
} from "$lib/i18n/m/privacy";
import type { MarketingLocale } from "$lib/i18n/marketing-copy";

/** Privacy policy copy. Imported only by the privacy route, so the terms body never reaches it. */
export function resolvePrivacyCopy(locale: MarketingLocale): LegalPageCopy {
  return {
    ...resolveLegalChrome(locale),
    seo: {
      title: legal_privacy_seo_title(undefined, { locale }),
      description: legal_privacy_seo_description(undefined, { locale }),
      imageAlt: legal_privacy_seo_title(undefined, { locale }),
    },
    heading: legal_privacy_heading(undefined, { locale }),
    sections: [
      {
        id: "collect",
        heading: legal_privacy_collect_heading(undefined, { locale }),
        body: legal_privacy_collect_body(undefined, { locale }),
      },
      {
        id: "accounts",
        heading: legal_privacy_accounts_heading(undefined, { locale }),
        body: legal_privacy_accounts_body(undefined, { locale }),
      },
      {
        id: "device",
        heading: legal_privacy_device_heading(undefined, { locale }),
        body: legal_privacy_device_body(undefined, { locale }),
      },
      {
        id: "hosting",
        heading: legal_privacy_hosting_heading(undefined, { locale }),
        body: legal_privacy_hosting_body(undefined, { locale }),
      },
      {
        id: "notifications",
        heading: legal_privacy_notifications_heading(undefined, { locale }),
        body: legal_privacy_notifications_body(undefined, { locale }),
      },
      {
        id: "children",
        heading: legal_privacy_children_heading(undefined, { locale }),
        body: legal_privacy_children_body(undefined, { locale }),
      },
      {
        id: "choices",
        heading: legal_privacy_choices_heading(undefined, { locale }),
        body: legal_privacy_choices_body(undefined, { locale }),
      },
      {
        id: "changes",
        heading: legal_privacy_changes_heading(undefined, { locale }),
        body: legal_privacy_changes_body(undefined, { locale }),
      },
    ],
    outroPrompt: legal_privacy_questions_prompt(undefined, { locale }),
    outroLink: legal_privacy_questions_link(undefined, { locale }),
  };
}

/** The privacy summary panel, which only the privacy page renders. */
export function resolvePrivacySummary(locale: MarketingLocale): string {
  return legal_privacy_summary(undefined, { locale });
}
