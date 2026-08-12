import type { IconName } from "$lib/components/icon";
import type { AccentId, SurfaceId, ThemeMode } from "$lib/config/site";
import { SUPPORTED_UI_LOCALES, UI_LOCALES, uiDirection, type UiLocale } from "$lib/i18n/locales";
import { marketingHref } from "$lib/i18n/marketing";
import { readerHomeHrefFor } from "$lib/i18n/reader";
import { m } from "$lib/paraglide/messages.js";

export const MARKETING_LOCALES = SUPPORTED_UI_LOCALES;
export type MarketingLocale = UiLocale;
export type MarketingDirection = "ltr" | "rtl";

export interface LocaleLink {
  locale: MarketingLocale;
  direction: MarketingDirection;
  label: string;
  href: `/${string}`;
  current: boolean;
}

export interface BrandResolvedCopy {
  homeLabel: string;
}

export interface NavResolvedCopy {
  primaryLabel: string;
  offlineLabel: string;
  offlineTitle: string;
  offlineDetail: string;
  searchQuran: string;
  account: string;
  signIn: string;
  openPanel: string;
  closePanel: string;
  sitePanel: string;
  appearance: string;
  toggleTheme: string;
  theme: string;
  language: string;
  changeLanguage: string;
  themeNames: Record<ThemeMode, string>;
}

export interface FooterResolvedCopy {
  blurb: string;
  socialX: string;
  productHeading: string;
  productLabel: string;
  companyHeading: string;
  companyLabel: string;
  legalHeading: string;
  legalLabel: string;
  builtBy: string;
  projectBy: string;
}

export interface FooterLink {
  id: string;
  href: `/${string}`;
  label: string;
}

export interface MarketingFooterLinks {
  product: FooterLink[];
  company: FooterLink[];
  legal: FooterLink[];
}

export interface SurfaceResolvedCopy {
  label: string;
  note: string;
}

export interface TweaksResolvedCopy {
  settings: string;
  theme: string;
  closePanel: string;
  mode: string;
  surface: string;
  accent: string;
  customColours: string;
  clear: string;
  seedNames: Record<"bg" | "accent" | "pop", string>;
  colourLabel: string;
  accentOptionLabel: (name: string) => string;
  colourInputLabel: (name: string) => string;
  preset: string;
  resetToPresetLabel: (name: string) => string;
  toggleStatusLabel: (name: string, status: string) => string;
  derivedColours: string;
  copied: string;
  copyCss: string;
  reset: string;
  dataPrivacy: string;
  analytics: string;
  performance: string;
  on: string;
  off: string;
  performanceReload: string;
  customizeAppearance: string;
  themeNames: Record<ThemeMode, string>;
  surfaces: Record<SurfaceId, SurfaceResolvedCopy>;
  accents: Record<AccentId, string>;
}

export interface LandingCard {
  id: string;
  icon: IconName;
  chip: string;
  title: string;
  body: string;
}

export interface LandingRoadmapItem {
  id: string;
  title: string;
  body: string;
}

export interface LandingResolvedCopy {
  badge: string;
  heroTitle: string;
  heroIntro: string;
  primaryCta: string;
  secondaryCta: string;
  todayEyebrow: string;
  todayTitle: string;
  todayIntro: string;
  values: LandingCard[];
  roadmapEyebrow: string;
  roadmapTitle: string;
  roadmapIntro: string;
  coming: string;
  roadmap: LandingRoadmapItem[];
}

export interface MarketingSeoCopy {
  title: string;
  description: string;
  imageAlt: string;
}

export interface MarketingResolvedCopy {
  locale: MarketingLocale;
  direction: MarketingDirection;
  skipToContent: string;
  brand: BrandResolvedCopy;
  nav: NavResolvedCopy;
  footer: FooterResolvedCopy;
  tweaks: TweaksResolvedCopy;
  landing: LandingResolvedCopy;
  seo: MarketingSeoCopy;
}

export function marketingLocaleFromPath(pathname: string): MarketingLocale {
  return pathname === "/ar" || pathname.startsWith("/ar/") ? "ar" : "en";
}

export function marketingDirection(locale: MarketingLocale): MarketingDirection {
  return uiDirection(locale);
}

export function marketingHomeHref(locale: MarketingLocale): "/" | "/ar/" {
  return marketingHref("home", locale) as "/" | "/ar/";
}

export function marketingReaderHomeHref(locale: MarketingLocale): "/en/app" | "/ar/app" {
  return readerHomeHrefFor(locale);
}

export function marketingLocaleLinks(current: MarketingLocale): LocaleLink[] {
  return MARKETING_LOCALES.map((locale) => ({
    locale,
    direction: UI_LOCALES[locale].direction,
    label: UI_LOCALES[locale].endonym,
    href: marketingHomeHref(locale),
    current: locale === current,
  }));
}

export function marketingFooterLinks(locale: MarketingLocale): MarketingFooterLinks {
  const home = marketingHomeHref(locale);
  const reader = marketingReaderHomeHref(locale);

  return {
    product: [
      { id: "read", href: reader, label: m.footer_read_quran(undefined, { locale }) },
      { id: "bookmarks", href: reader, label: m.footer_bookmarks(undefined, { locale }) },
      {
        id: "inside",
        href: `${home}#today` as `/${string}`,
        label: m.footer_whats_inside(undefined, { locale }),
      },
    ],
    company:
      locale === "en"
        ? [
            { id: "about", href: "/about", label: m.footer_about(undefined, { locale }) },
            { id: "faq", href: "/faq", label: m.footer_faq(undefined, { locale }) },
            { id: "contact", href: "/contact", label: m.footer_contact(undefined, { locale }) },
          ]
        : [],
    legal:
      locale === "en"
        ? [
            { id: "privacy", href: "/privacy", label: m.footer_privacy(undefined, { locale }) },
            { id: "terms", href: "/terms", label: m.footer_terms(undefined, { locale }) },
          ]
        : [],
  };
}

export function resolveMarketingCopy(locale: MarketingLocale): MarketingResolvedCopy {
  return {
    locale,
    direction: marketingDirection(locale),
    skipToContent: m.skip_to_content(undefined, { locale }),
    brand: { homeLabel: m.brand_home_label(undefined, { locale }) },
    nav: {
      primaryLabel: m.nav_primary_label(undefined, { locale }),
      offlineLabel: m.nav_offline_label(undefined, { locale }),
      offlineTitle: m.nav_offline_title(undefined, { locale }),
      offlineDetail: m.nav_offline_detail(undefined, { locale }),
      searchQuran: m.nav_search_quran(undefined, { locale }),
      account: m.nav_account(undefined, { locale }),
      signIn: m.nav_sign_in(undefined, { locale }),
      openPanel: m.nav_open_panel(undefined, { locale }),
      closePanel: m.nav_close_panel(undefined, { locale }),
      sitePanel: m.nav_site_panel(undefined, { locale }),
      appearance: m.nav_appearance(undefined, { locale }),
      toggleTheme: m.nav_toggle_theme(undefined, { locale }),
      theme: m.nav_theme(undefined, { locale }),
      language: m.nav_language(undefined, { locale }),
      changeLanguage: m.nav_change_language(undefined, { locale }),
      themeNames: {
        dark: m.theme_dark(undefined, { locale }),
        light: m.theme_light(undefined, { locale }),
      },
    },
    footer: {
      blurb: m.footer_blurb(undefined, { locale }),
      socialX: m.footer_social_x(undefined, { locale }),
      productHeading: m.footer_product_heading(undefined, { locale }),
      productLabel: m.footer_product_label(undefined, { locale }),
      companyHeading: m.footer_company_heading(undefined, { locale }),
      companyLabel: m.footer_company_label(undefined, { locale }),
      legalHeading: m.footer_legal_heading(undefined, { locale }),
      legalLabel: m.footer_legal_label(undefined, { locale }),
      builtBy: m.footer_built_by(undefined, { locale }),
      projectBy: m.footer_project_by(undefined, { locale }),
    },
    tweaks: {
      settings: m.tweaks_settings(undefined, { locale }),
      theme: m.tweaks_theme(undefined, { locale }),
      closePanel: m.tweaks_close_panel(undefined, { locale }),
      mode: m.tweaks_mode(undefined, { locale }),
      surface: m.tweaks_surface(undefined, { locale }),
      accent: m.tweaks_accent(undefined, { locale }),
      customColours: m.tweaks_custom_colours(undefined, { locale }),
      clear: m.tweaks_clear(undefined, { locale }),
      seedNames: {
        bg: m.tweaks_background(undefined, { locale }),
        accent: m.tweaks_accent(undefined, { locale }),
        pop: m.tweaks_pop(undefined, { locale }),
      },
      colourLabel: m.tweaks_colour_label(undefined, { locale }),
      accentOptionLabel: (name) => m.tweaks_accent_option({ name }, { locale }),
      colourInputLabel: (name) => m.tweaks_colour_input({ name }, { locale }),
      preset: m.tweaks_preset(undefined, { locale }),
      resetToPresetLabel: (name) => m.tweaks_reset_to_preset({ name }, { locale }),
      toggleStatusLabel: (name, status) => m.tweaks_toggle_status({ name, status }, { locale }),
      derivedColours: m.tweaks_derived_colours(undefined, { locale }),
      copied: m.tweaks_copied(undefined, { locale }),
      copyCss: m.tweaks_copy_css(undefined, { locale }),
      reset: m.tweaks_reset(undefined, { locale }),
      dataPrivacy: m.tweaks_data_privacy(undefined, { locale }),
      analytics: m.tweaks_analytics(undefined, { locale }),
      performance: m.tweaks_performance(undefined, { locale }),
      on: m.tweaks_on(undefined, { locale }),
      off: m.tweaks_off(undefined, { locale }),
      performanceReload: m.tweaks_performance_reload(undefined, { locale }),
      customizeAppearance: m.tweaks_customize_appearance(undefined, { locale }),
      themeNames: {
        dark: m.theme_dark(undefined, { locale }),
        light: m.theme_light(undefined, { locale }),
      },
      surfaces: {
        ink: {
          label: m.surface_ink_label(undefined, { locale }),
          note: m.surface_ink_note(undefined, { locale }),
        },
        paper: {
          label: m.surface_paper_label(undefined, { locale }),
          note: m.surface_paper_note(undefined, { locale }),
        },
        slate: {
          label: m.surface_slate_label(undefined, { locale }),
          note: m.surface_slate_note(undefined, { locale }),
        },
        mocha: {
          label: m.surface_mocha_label(undefined, { locale }),
          note: m.surface_mocha_note(undefined, { locale }),
        },
        contrast: {
          label: m.surface_contrast_label(undefined, { locale }),
          note: m.surface_contrast_note(undefined, { locale }),
        },
      },
      accents: {
        emerald: m.accent_emerald_label(undefined, { locale }),
        gold: m.accent_gold_label(undefined, { locale }),
        azure: m.accent_azure_label(undefined, { locale }),
        plum: m.accent_plum_label(undefined, { locale }),
      },
    },
    landing: {
      badge: m.landing_badge(undefined, { locale }),
      heroTitle: m.landing_hero_title(undefined, { locale }),
      heroIntro: m.landing_hero_intro(undefined, { locale }),
      primaryCta: m.landing_primary_cta(undefined, { locale }),
      secondaryCta: m.landing_secondary_cta(undefined, { locale }),
      todayEyebrow: m.landing_today_eyebrow(undefined, { locale }),
      todayTitle: m.landing_today_title(undefined, { locale }),
      todayIntro: m.landing_today_intro(undefined, { locale }),
      values: [
        {
          id: "instant",
          icon: "arrow-right",
          chip: "bg-accent text-accent-fg",
          title: m.landing_value_instant_title(undefined, { locale }),
          body: m.landing_value_instant_body(undefined, { locale }),
        },
        {
          id: "size",
          icon: "plus",
          chip: "bg-fg text-bg",
          title: m.landing_value_size_title(undefined, { locale }),
          body: m.landing_value_size_body(undefined, { locale }),
        },
        {
          id: "free",
          icon: "check",
          chip: "bg-pop text-accent-fg",
          title: m.landing_value_free_title(undefined, { locale }),
          body: m.landing_value_free_body(undefined, { locale }),
        },
        {
          id: "authentic",
          icon: "book",
          chip: "bg-accent-soft text-accent",
          title: m.landing_value_authentic_title(undefined, { locale }),
          body: m.landing_value_authentic_body(undefined, { locale }),
        },
      ],
      roadmapEyebrow: m.landing_roadmap_eyebrow(undefined, { locale }),
      roadmapTitle: m.landing_roadmap_title(undefined, { locale }),
      roadmapIntro: m.landing_roadmap_intro(undefined, { locale }),
      coming: m.landing_coming(undefined, { locale }),
      roadmap: [
        {
          id: "native",
          title: m.landing_roadmap_native_title(undefined, { locale }),
          body: m.landing_roadmap_native_body(undefined, { locale }),
        },
        {
          id: "sync",
          title: m.landing_roadmap_sync_title(undefined, { locale }),
          body: m.landing_roadmap_sync_body(undefined, { locale }),
        },
        {
          id: "audio",
          title: m.landing_roadmap_audio_title(undefined, { locale }),
          body: m.landing_roadmap_audio_body(undefined, { locale }),
        },
        {
          id: "content",
          title: m.landing_roadmap_content_title(undefined, { locale }),
          body: m.landing_roadmap_content_body(undefined, { locale }),
        },
      ],
    },
    seo: {
      title: m.seo_home_title(undefined, { locale }),
      description: m.seo_home_description(undefined, { locale }),
      imageAlt: m.seo_home_image_alt(undefined, { locale }),
    },
  };
}
