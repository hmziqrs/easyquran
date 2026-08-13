import {
  contact_email_body,
  contact_email_title,
  contact_eyebrow,
  contact_heading,
  contact_intro,
  contact_reply_label,
  contact_reply_value,
  contact_seo_description,
  contact_seo_title,
  contact_x_body,
  contact_x_title,
} from "$lib/i18n/m/contact";
import type { MarketingLocale, MarketingSeoCopy } from "$lib/i18n/marketing-copy";

export interface ContactResolvedCopy {
  seo: MarketingSeoCopy;
  eyebrow: string;
  heading: string;
  intro: string;
  emailTitle: string;
  emailBody: string;
  xTitle: string;
  xBody: string;
  replyLabel: string;
  replyValue: string;
}

/** Contact page copy. Imported only by the contact route, so it chunks with that route. */
export function resolveContactCopy(locale: MarketingLocale): ContactResolvedCopy {
  return {
    seo: {
      title: contact_seo_title(undefined, { locale }),
      description: contact_seo_description(undefined, { locale }),
      imageAlt: contact_seo_title(undefined, { locale }),
    },
    eyebrow: contact_eyebrow(undefined, { locale }),
    heading: contact_heading(undefined, { locale }),
    intro: contact_intro(undefined, { locale }),
    emailTitle: contact_email_title(undefined, { locale }),
    emailBody: contact_email_body(undefined, { locale }),
    xTitle: contact_x_title(undefined, { locale }),
    xBody: contact_x_body(undefined, { locale }),
    replyLabel: contact_reply_label(undefined, { locale }),
    replyValue: contact_reply_value(undefined, { locale }),
  };
}
