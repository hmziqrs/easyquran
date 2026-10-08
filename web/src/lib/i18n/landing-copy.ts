import {
  landing_bismillah,
  landing_hero_title_full,
  landing_index_juz,
  landing_index_label,
  landing_index_pages,
  landing_index_surahs,
  landing_index_yours,
  landing_often_opened,
  landing_search_button,
  landing_search_label,
  landing_search_placeholder,
  landing_surah_ayahs,
  seo_home_description,
  seo_home_image_alt,
  seo_home_title,
} from "#lib/i18n/m/landing.js";
import type {
  LandingResolvedCopy,
  MarketingLocale,
  MarketingSeoCopy,
} from "#lib/i18n/marketing-copy.js";

/** Marketing home page copy. Imported only by the landing route, so it chunks with that route. */
export function resolveLandingCopy(locale: MarketingLocale): LandingResolvedCopy {
  const options = { locale } as const;
  return {
    heroTitleFull: landing_hero_title_full(undefined, options),
    bismillah: landing_bismillah(undefined, options),
    searchLabel: landing_search_label(undefined, options),
    searchPlaceholder: landing_search_placeholder(undefined, options),
    searchButton: landing_search_button(undefined, options),
    oftenOpened: landing_often_opened(undefined, options),
    indexLabel: landing_index_label(undefined, options),
    indexSurahs: landing_index_surahs(undefined, options),
    indexJuz: landing_index_juz(undefined, options),
    indexPages: landing_index_pages(undefined, options),
    indexYours: landing_index_yours(undefined, options),
    ayahCount: (count) => landing_surah_ayahs({ count }, options),
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
