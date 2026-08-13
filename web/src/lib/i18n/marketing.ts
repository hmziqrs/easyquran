import { MARKETING_ROUTES } from "$lib/config/site-structure";
import { assertUiLocale, type UiLocale } from "$lib/i18n/locales";
import { localizeHref } from "$lib/paraglide/runtime";

export const MARKETING_PATHS = MARKETING_ROUTES;

export type MarketingPageId = keyof typeof MARKETING_PATHS;
export type MarketingPath = (typeof MARKETING_PATHS)[MarketingPageId];
export type MarketingHref = MarketingPath | `/${Exclude<UiLocale, "en">}${MarketingPath}`;

const ENGLISH_ONLY = Object.freeze(["en"] as const);
const ENGLISH_AND_ARABIC = Object.freeze(["en", "ar"] as const);

export const MARKETING_PUBLICATIONS = Object.freeze({
  home: ENGLISH_AND_ARABIC,
  about: ENGLISH_ONLY,
  faq: ENGLISH_ONLY,
  contact: ENGLISH_ONLY,
  privacy: ENGLISH_ONLY,
  terms: ENGLISH_ONLY,
} as const satisfies Readonly<Record<MarketingPageId, readonly UiLocale[]>>);

// eslint-disable-next-line anti-slop/no-unknown-parameters -- exported runtime type guard; accepts opaque page IDs (URL params, user input) and parses via Object.hasOwn below.
export function isMarketingPageId(value: unknown): value is MarketingPageId {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- first parse step of the guard: discriminate strings before the Object.hasOwn lookup.
  return typeof value === "string" && Object.hasOwn(MARKETING_PATHS, value);
}

export function publishedMarketingLocales(pageId: MarketingPageId): readonly UiLocale[] {
  if (!isMarketingPageId(pageId)) return Object.freeze([]);
  return MARKETING_PUBLICATIONS[pageId];
}

export function isMarketingPublished(pageId: MarketingPageId, locale: UiLocale): boolean {
  assertUiLocale(locale);
  return isMarketingPageId(pageId) && publishedMarketingLocales(pageId).includes(locale);
}

export function marketingHref(pageId: MarketingPageId, locale: UiLocale): MarketingHref | null {
  assertUiLocale(locale);
  if (!isMarketingPublished(pageId, locale)) return null;

  const localized = localizeHref(MARKETING_PATHS[pageId], { locale });
  if (!localized.startsWith("/") || localized.startsWith("//") || /[?#]/.test(localized)) {
    throw new Error(`Invalid localized marketing href: ${localized}`);
  }
  // SAFETY: the startsWith("/") + not-"//" + no-`?#` regex checks above prove `localized` is a canonical marketing path with no query/fragment, matching MarketingHref.
  return localized as MarketingHref;
}
