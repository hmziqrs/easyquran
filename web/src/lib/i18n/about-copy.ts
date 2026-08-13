import {
  about_body_approach,
  about_body_sources,
  about_body_today,
  about_credit_built_by,
  about_credit_note,
  about_credit_project_by,
  about_cta,
  about_eyebrow,
  about_heading,
  about_intro,
  about_seo_description,
  about_seo_title,
  about_sources_heading,
  about_sources_lead,
  about_sources_tail,
  about_sources_tanzil_label,
  about_stat_free_label,
  about_stat_free_value,
  about_stat_growing_label,
  about_stat_growing_value,
  about_stat_script_label,
  about_stat_script_value,
} from "$lib/i18n/m/about";
import type { MarketingLocale, MarketingSeoCopy } from "$lib/i18n/marketing-copy";

export interface AboutStat {
  /** Stable across locales so the DOM key never depends on translated text. */
  id: "free" | "script" | "growing";
  value: string;
  label: string;
}

export interface AboutResolvedCopy {
  seo: MarketingSeoCopy;
  eyebrow: string;
  heading: string;
  intro: string;
  paragraphs: { id: string; body: string }[];
  stats: AboutStat[];
  creditProjectBy: string;
  creditBuiltBy: string;
  creditNote: string;
  cta: string;
  sourcesHeading: string;
  sourcesLead: string;
  sourcesTanzilLabel: string;
  sourcesTail: string;
}

/** About page copy. Imported only by the about route, so it chunks with that route. */
export function resolveAboutCopy(locale: MarketingLocale): AboutResolvedCopy {
  return {
    seo: {
      title: about_seo_title(undefined, { locale }),
      description: about_seo_description(undefined, { locale }),
      imageAlt: about_seo_title(undefined, { locale }),
    },
    eyebrow: about_eyebrow(undefined, { locale }),
    heading: about_heading(undefined, { locale }),
    intro: about_intro(undefined, { locale }),
    paragraphs: [
      { id: "approach", body: about_body_approach(undefined, { locale }) },
      { id: "today", body: about_body_today(undefined, { locale }) },
      { id: "sources", body: about_body_sources(undefined, { locale }) },
    ],
    stats: [
      {
        id: "free",
        value: about_stat_free_value(undefined, { locale }),
        label: about_stat_free_label(undefined, { locale }),
      },
      {
        id: "script",
        value: about_stat_script_value(undefined, { locale }),
        label: about_stat_script_label(undefined, { locale }),
      },
      {
        id: "growing",
        value: about_stat_growing_value(undefined, { locale }),
        label: about_stat_growing_label(undefined, { locale }),
      },
    ],
    creditProjectBy: about_credit_project_by(undefined, { locale }),
    creditBuiltBy: about_credit_built_by(undefined, { locale }),
    creditNote: about_credit_note(undefined, { locale }),
    cta: about_cta(undefined, { locale }),
    sourcesHeading: about_sources_heading(undefined, { locale }),
    sourcesLead: about_sources_lead(undefined, { locale }),
    sourcesTanzilLabel: about_sources_tanzil_label(undefined, { locale }),
    sourcesTail: about_sources_tail(undefined, { locale }),
  };
}
