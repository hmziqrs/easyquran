const UI_LOCALE_IDS = ["en", "ar"] as const;
export type UiLocale = (typeof UI_LOCALE_IDS)[number];

export const DEFAULT_UI_LOCALE = "en" as const satisfies UiLocale;

const ENGLISH_UI_LOCALE = Object.freeze({
  locale: "en",
  endonym: "English",
  direction: "ltr",
  openGraphLocale: "en_US",
} as const);

const ARABIC_UI_LOCALE = Object.freeze({
  locale: "ar",
  endonym: "العربية",
  direction: "rtl",
  openGraphLocale: "ar_SA",
} as const);

export const UI_LOCALES = Object.freeze({
  en: ENGLISH_UI_LOCALE,
  ar: ARABIC_UI_LOCALE,
} as const satisfies Readonly<Record<UiLocale, {
  locale: string;
  endonym: string;
  direction: string;
  openGraphLocale: string;
}>>);

export type UiDirection = (typeof UI_LOCALES)[UiLocale]["direction"];
export type UiLocaleMetadata = (typeof UI_LOCALES)[UiLocale];

export const SUPPORTED_UI_LOCALES = Object.freeze([...UI_LOCALE_IDS]);

// eslint-disable-next-line anti-slop/no-unknown-parameters -- exported runtime type guard; accepts opaque locale values (URL params, paraglide getLocale()) and parses via the Object.hasOwn lookup below.
export function isUiLocale(value: unknown): value is UiLocale {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- first parse step of the guard: discriminate strings before the Object.hasOwn lookup.
  return typeof value === "string" && Object.hasOwn(UI_LOCALES, value);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- assertion counterpart to isUiLocale; takes the same opaque input it is about to prove or reject.
export function assertUiLocale(value: unknown): asserts value is UiLocale {
  if (!isUiLocale(value)) throw new TypeError(`Unsupported UI locale: ${String(value)}`);
}

export function uiLocaleMetadata(locale: UiLocale): UiLocaleMetadata {
  assertUiLocale(locale);
  return UI_LOCALES[locale];
}

export function uiDirection(locale: UiLocale): UiDirection {
  return uiLocaleMetadata(locale).direction;
}
