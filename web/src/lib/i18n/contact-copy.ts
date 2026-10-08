import {
  contact_email_title,
  contact_heading,
  contact_intro,
  contact_seo_description,
  contact_seo_title,
  contact_x_title,
} from "#lib/i18n/m/contact.js";
import type { MarketingLocale, MarketingSeoCopy } from "#lib/i18n/marketing-copy.js";

export interface ContactResolvedCopy {
  seo: MarketingSeoCopy;
  heading: string;
  intro: string;
  emailTitle: string;
  xTitle: string;
}

/** Contact page copy. Imported only by the contact route, so it chunks with that route. */
export function resolveContactCopy(locale: MarketingLocale): ContactResolvedCopy {
  return {
    seo: {
      title: contact_seo_title(undefined, { locale }),
      description: contact_seo_description(undefined, { locale }),
      imageAlt: contact_seo_title(undefined, { locale }),
    },
    heading: contact_heading(undefined, { locale }),
    intro: contact_intro(undefined, { locale }),
    emailTitle: contact_email_title(undefined, { locale }),
    xTitle: contact_x_title(undefined, { locale }),
  };
}
