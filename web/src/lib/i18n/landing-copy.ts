import type {
  LandingResolvedCopy,
  MarketingLocale,
  MarketingSeoCopy,
} from "$lib/i18n/marketing-copy";
import {
  landing_badge,
  landing_coming,
  landing_hero_intro,
  landing_hero_title,
  landing_primary_cta,
  landing_roadmap_audio_body,
  landing_roadmap_audio_title,
  landing_roadmap_content_body,
  landing_roadmap_content_title,
  landing_roadmap_eyebrow,
  landing_roadmap_intro,
  landing_roadmap_native_body,
  landing_roadmap_native_title,
  landing_roadmap_sync_body,
  landing_roadmap_sync_title,
  landing_roadmap_title,
  landing_secondary_cta,
  landing_today_eyebrow,
  landing_today_intro,
  landing_today_title,
  landing_value_authentic_body,
  landing_value_authentic_title,
  landing_value_free_body,
  landing_value_free_title,
  landing_value_instant_body,
  landing_value_instant_title,
  landing_value_size_body,
  landing_value_size_title,
  seo_home_description,
  seo_home_image_alt,
  seo_home_title,
} from "$lib/i18n/m/landing";

/** Marketing home page copy. Imported only by the landing route, so it chunks with that route. */
export function resolveLandingCopy(locale: MarketingLocale): LandingResolvedCopy {
  return {
    badge: landing_badge(undefined, { locale }),
    heroTitle: landing_hero_title(undefined, { locale }),
    heroIntro: landing_hero_intro(undefined, { locale }),
    primaryCta: landing_primary_cta(undefined, { locale }),
    secondaryCta: landing_secondary_cta(undefined, { locale }),
    todayEyebrow: landing_today_eyebrow(undefined, { locale }),
    todayTitle: landing_today_title(undefined, { locale }),
    todayIntro: landing_today_intro(undefined, { locale }),
    values: [
      {
        id: "instant",
        icon: "arrow-right",
        chip: "bg-accent text-accent-fg",
        title: landing_value_instant_title(undefined, { locale }),
        body: landing_value_instant_body(undefined, { locale }),
      },
      {
        id: "size",
        icon: "plus",
        chip: "bg-fg text-bg",
        title: landing_value_size_title(undefined, { locale }),
        body: landing_value_size_body(undefined, { locale }),
      },
      {
        id: "free",
        icon: "check",
        chip: "bg-pop text-accent-fg",
        title: landing_value_free_title(undefined, { locale }),
        body: landing_value_free_body(undefined, { locale }),
      },
      {
        id: "authentic",
        icon: "book",
        chip: "bg-accent-soft text-accent",
        title: landing_value_authentic_title(undefined, { locale }),
        body: landing_value_authentic_body(undefined, { locale }),
      },
    ],
    roadmapEyebrow: landing_roadmap_eyebrow(undefined, { locale }),
    roadmapTitle: landing_roadmap_title(undefined, { locale }),
    roadmapIntro: landing_roadmap_intro(undefined, { locale }),
    coming: landing_coming(undefined, { locale }),
    roadmap: [
      {
        id: "native",
        title: landing_roadmap_native_title(undefined, { locale }),
        body: landing_roadmap_native_body(undefined, { locale }),
      },
      {
        id: "sync",
        title: landing_roadmap_sync_title(undefined, { locale }),
        body: landing_roadmap_sync_body(undefined, { locale }),
      },
      {
        id: "audio",
        title: landing_roadmap_audio_title(undefined, { locale }),
        body: landing_roadmap_audio_body(undefined, { locale }),
      },
      {
        id: "content",
        title: landing_roadmap_content_title(undefined, { locale }),
        body: landing_roadmap_content_body(undefined, { locale }),
      },
    ],
  };
}

/** Landing metadata. Other marketing pages own their own SEO namespace. */
export function resolveLandingSeoCopy(locale: MarketingLocale): MarketingSeoCopy {
  return {
    title: seo_home_title(undefined, { locale }),
    description: seo_home_description(undefined, { locale }),
    imageAlt: seo_home_image_alt(undefined, { locale }),
  };
}
