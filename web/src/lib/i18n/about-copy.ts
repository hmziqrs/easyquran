import {
  about_credit_built_by,
  about_credit_project_by,
  about_cta,
  about_feature_scripts,
  about_feature_tools,
  about_feature_translations,
  about_heading,
  about_intro,
  about_seo_description,
  about_seo_title,
  about_sources_arabic,
  about_sources_heading,
  about_sources_note,
  about_sources_translations,
} from "#lib/i18n/m/about.js";
import type { MarketingLocale, MarketingSeoCopy } from "#lib/i18n/marketing-copy.js";

export interface AboutFeature {
  /** Stable across locales so the DOM key never depends on translated text. */
  id: "scripts" | "translations" | "tools";
  body: string;
}

/** Catalogue totals, counted at build time so the page never states a stale number. */
export interface AboutCounts {
  translations: number;
  languages: number;
}

export interface AboutResolvedCopy {
  seo: MarketingSeoCopy;
  heading: string;
  intro: string;
  features: AboutFeature[];
  sourcesHeading: string;
  sourcesArabic: string;
  sourcesTranslations: string;
  sourcesNote: string;
  creditProjectBy: string;
  creditBuiltBy: string;
  cta: string;
}

/** About page copy. Imported only by the about route, so it chunks with that route. */
export function resolveAboutCopy(locale: MarketingLocale, counts: AboutCounts): AboutResolvedCopy {
  const options = { locale } as const;
  return {
    seo: {
      title: about_seo_title(undefined, options),
      description: about_seo_description(undefined, options),
      imageAlt: about_seo_title(undefined, options),
    },
    heading: about_heading(undefined, options),
    intro: about_intro(undefined, options),
    features: [
      { id: "scripts", body: about_feature_scripts(undefined, options) },
      {
        id: "translations",
        body: about_feature_translations(
          { translations: String(counts.translations), languages: String(counts.languages) },
          options,
        ),
      },
      { id: "tools", body: about_feature_tools(undefined, options) },
    ],
    sourcesHeading: about_sources_heading(undefined, options),
    sourcesArabic: about_sources_arabic(undefined, options),
    sourcesTranslations: about_sources_translations(undefined, options),
    sourcesNote: about_sources_note(undefined, options),
    creditProjectBy: about_credit_project_by(undefined, options),
    creditBuiltBy: about_credit_built_by(undefined, options),
    cta: about_cta(undefined, options),
  };
}
