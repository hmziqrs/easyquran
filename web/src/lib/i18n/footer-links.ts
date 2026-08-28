// Single source for the footer link columns. Both surfaces render the same three columns from the
// same spec: marketing pages resolve through `marketingHref`, so an unpublished locale drops the
// link instead of hard-coding "en". Callers supply the labels (message-free module), the reader
// href, which differs per surface: marketing links at the reader home, the reader keeps its
// current translation context, and the bookmarks-page href (canonical unlocalized /app/bookmarks —
// localized /{en,ar}/app/bookmarks is not a published route; see bookmarksPageHref).
import { marketingHref } from "$lib/i18n/marketing";
import type { MarketingPageId } from "$lib/i18n/marketing";
import type { FooterLink, MarketingFooterLinks, MarketingLocale } from "$lib/i18n/marketing-copy";

export interface FooterLinkLabels {
  readonly readQuran: string;
  readonly bookmarks: string;
  readonly whatsInside: string;
  readonly about: string;
  readonly faq: string;
  readonly contact: string;
  readonly privacy: string;
  readonly terms: string;
}

interface MarketingLinkSpec {
  readonly id: string;
  readonly page: MarketingPageId;
  readonly label: keyof FooterLinkLabels;
}

const COMPANY_LINKS = [
  { id: "about", page: "about", label: "about" },
  { id: "faq", page: "faq", label: "faq" },
  { id: "contact", page: "contact", label: "contact" },
] as const satisfies readonly MarketingLinkSpec[];

const LEGAL_LINKS = [
  { id: "privacy", page: "privacy", label: "privacy" },
  { id: "terms", page: "terms", label: "terms" },
] as const satisfies readonly MarketingLinkSpec[];

function marketingColumn(
  specs: readonly MarketingLinkSpec[],
  locale: MarketingLocale,
  labels: FooterLinkLabels,
): FooterLink[] {
  return specs.flatMap(({ id, page, label }) => {
    const href = marketingHref(page, locale);
    return href ? [{ id, href, label: labels[label] }] : [];
  });
}

export function footerLinksFor(
  locale: MarketingLocale,
  labels: FooterLinkLabels,
  readerHref: `/${string}`,
  bookmarksHref: `/${string}`,
): MarketingFooterLinks {
  const home = marketingHref("home", locale) ?? "/";

  return {
    product: [
      { id: "read", href: readerHref, label: labels.readQuran },
      { id: "bookmarks", href: bookmarksHref, label: labels.bookmarks },
      // "What's inside" targets the landing's why band (plan 05) — the old #today
      // section is gone; the why band is the reader-contents story now.
      { id: "inside", href: `${home}#why`, label: labels.whatsInside },
    ],
    company: marketingColumn(COMPANY_LINKS, locale, labels),
    legal: marketingColumn(LEGAL_LINKS, locale, labels),
  };
}
