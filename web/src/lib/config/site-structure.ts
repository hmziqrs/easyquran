export const MARKETING_ROUTES = Object.freeze({
  home: "/",
  about: "/about",
  faq: "/faq",
  contact: "/contact",
  privacy: "/privacy",
  terms: "/terms",
} as const);

export type MarketingPageId = keyof typeof MARKETING_ROUTES;

export interface SitePage<Id extends string = MarketingPageId> {
  readonly id: Id;
  readonly href: string;
  readonly nav?: boolean;
}

export const MARKETING_PAGES = Object.freeze([
  { id: "home", href: MARKETING_ROUTES.home, nav: true },
  { id: "about", href: MARKETING_ROUTES.about, nav: true },
  { id: "faq", href: MARKETING_ROUTES.faq, nav: true },
  { id: "contact", href: MARKETING_ROUTES.contact, nav: true },
  { id: "privacy", href: MARKETING_ROUTES.privacy },
  { id: "terms", href: MARKETING_ROUTES.terms },
] as const satisfies readonly SitePage<MarketingPageId>[]);
