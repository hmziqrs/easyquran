import type { LegalPageCopy } from "$lib/i18n/legal-copy";
import { resolveLegalChrome } from "$lib/i18n/legal-copy";
import type { MarketingLocale } from "$lib/i18n/marketing-copy";
import {
  legal_terms_acceptance_body,
  legal_terms_acceptance_heading,
  legal_terms_accuracy_body,
  legal_terms_accuracy_heading,
  legal_terms_contact_body,
  legal_terms_contact_heading,
  legal_terms_content_body,
  legal_terms_content_heading,
  legal_terms_heading,
  legal_terms_liability_body,
  legal_terms_liability_heading,
  legal_terms_privacy_link,
  legal_terms_privacy_prompt,
  legal_terms_seo_description,
  legal_terms_seo_title,
  legal_terms_service_body,
  legal_terms_service_heading,
  legal_terms_third_party_body,
  legal_terms_third_party_heading,
  legal_terms_use_body,
  legal_terms_use_heading,
} from "$lib/i18n/m/terms";

/** Terms of service copy. Imported only by the terms route. */
export function resolveTermsCopy(locale: MarketingLocale): LegalPageCopy {
  return {
    ...resolveLegalChrome(locale),
    seo: {
      title: legal_terms_seo_title(undefined, { locale }),
      description: legal_terms_seo_description(undefined, { locale }),
      imageAlt: legal_terms_seo_title(undefined, { locale }),
    },
    heading: legal_terms_heading(undefined, { locale }),
    sections: [
      {
        id: "acceptance",
        heading: legal_terms_acceptance_heading(undefined, { locale }),
        body: legal_terms_acceptance_body(undefined, { locale }),
      },
      {
        id: "service",
        heading: legal_terms_service_heading(undefined, { locale }),
        body: legal_terms_service_body(undefined, { locale }),
      },
      {
        id: "accuracy",
        heading: legal_terms_accuracy_heading(undefined, { locale }),
        body: legal_terms_accuracy_body(undefined, { locale }),
      },
      {
        id: "content",
        heading: legal_terms_content_heading(undefined, { locale }),
        body: legal_terms_content_body(undefined, { locale }),
      },
      {
        id: "use",
        heading: legal_terms_use_heading(undefined, { locale }),
        body: legal_terms_use_body(undefined, { locale }),
      },
      {
        id: "third-party",
        heading: legal_terms_third_party_heading(undefined, { locale }),
        body: legal_terms_third_party_body(undefined, { locale }),
      },
      {
        id: "liability",
        heading: legal_terms_liability_heading(undefined, { locale }),
        body: legal_terms_liability_body(undefined, { locale }),
      },
      {
        id: "contact",
        heading: legal_terms_contact_heading(undefined, { locale }),
        body: legal_terms_contact_body(undefined, { locale }),
      },
    ],
    outroPrompt: legal_terms_privacy_prompt(undefined, { locale }),
    outroLink: legal_terms_privacy_link(undefined, { locale }),
  };
}
