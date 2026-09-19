import { QURAN } from "$lib/config/site";
import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
import { TRANSLATIONS } from "$lib/data/translations";

export const TRANSLATION_CATALOGUE: readonly TranslationCatalogueEntry[] = Object.freeze(
  TRANSLATIONS.map((translation) =>
    Object.freeze({
      id: translation.id,
      language: translation.language,
      languageCode: translation.languageCode,
      direction: translation.direction,
      name: translation.name,
      translator: translation.translator,
      sizeBytes: translation.sizeBytes,
      downloadUrl: `${QURAN.artifactBase}/${translation.artifactPath}`,
    }),
  ),
);

export const TRANSLATION_CATALOGUE_BY_ID: ReadonlyMap<string, TranslationCatalogueEntry> = new Map(
  TRANSLATION_CATALOGUE.map((entry) => [entry.id, entry]),
);

/** Provenance of a translation artifact, derivable from its catalogue id prefix. */
export type TranslationProvenance = "qul" | "quranenc" | "tanzil";

export function translationSourceOf(id: string): TranslationProvenance {
  if (id.startsWith("qul.")) return "qul";
  if (id.startsWith("quranenc.")) return "quranenc";
  return "tanzil";
}

/** Flag emoji + country name for a language code; the country name only feeds search. */
export interface LanguageFlag {
  readonly flag: string;
  readonly country: string;
}

// Curated per languageCode present in web/src/lib/data/translations.json (96 codes).
// Stateless / transnational languages fall back to the globe emoji and an empty
// country name (search skips empty fields).
const GLOBE = "\u{1F310}";
export const LANGUAGE_FLAGS: Readonly<Record<string, LanguageFlag>> = Object.freeze({
  aa: { flag: "\u{1F1EA}\u{1F1F9}", country: "Ethiopia" },
  ak: { flag: "\u{1F1EC}\u{1F1ED}", country: "Ghana" },
  am: { flag: "\u{1F1EA}\u{1F1F9}", country: "Ethiopia" },
  ar: { flag: "\u{1F1F8}\u{1F1E6}", country: "Saudi Arabia" },
  as: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  az: { flag: "\u{1F1E6}\u{1F1FF}", country: "Azerbaijan" },
  be: { flag: "\u{1F1E7}\u{1F1FE}", country: "Belarus" },
  ber: { flag: GLOBE, country: "" },
  bg: { flag: "\u{1F1E7}\u{1F1EC}", country: "Bulgaria" },
  bi: { flag: "\u{1F1FB}\u{1F1FA}", country: "Vanuatu" },
  bm: { flag: "\u{1F1F2}\u{1F1F1}", country: "Mali" },
  bn: { flag: "\u{1F1E7}\u{1F1E9}", country: "Bangladesh" },
  bs: { flag: "\u{1F1E7}\u{1F1E6}", country: "Bosnia and Herzegovina" },
  ceb: { flag: "\u{1F1F5}\u{1F1ED}", country: "Philippines" },
  cs: { flag: "\u{1F1E8}\u{1F1FF}", country: "Czechia" },
  dag: { flag: "\u{1F1EC}\u{1F1ED}", country: "Ghana" },
  de: { flag: "\u{1F1E9}\u{1F1EA}", country: "Germany" },
  dv: { flag: "\u{1F1F2}\u{1F1FB}", country: "Maldives" },
  el: { flag: "\u{1F1EC}\u{1F1F7}", country: "Greece" },
  en: { flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" },
  es: { flag: "\u{1F1EA}\u{1F1F8}", country: "Spain" },
  fa: { flag: "\u{1F1EE}\u{1F1F7}", country: "Iran" },
  ff: { flag: GLOBE, country: "" },
  fi: { flag: "\u{1F1EB}\u{1F1EE}", country: "Finland" },
  fil: { flag: "\u{1F1F5}\u{1F1ED}", country: "Philippines" },
  fr: { flag: "\u{1F1EB}\u{1F1F7}", country: "France" },
  gu: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  ha: { flag: "\u{1F1F3}\u{1F1EC}", country: "Nigeria" },
  he: { flag: "\u{1F1EE}\u{1F1F1}", country: "Israel" },
  hi: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  hr: { flag: "\u{1F1ED}\u{1F1F7}", country: "Croatia" },
  id: { flag: "\u{1F1EE}\u{1F1E9}", country: "Indonesia" },
  it: { flag: "\u{1F1EE}\u{1F1F9}", country: "Italy" },
  ja: { flag: "\u{1F1EF}\u{1F1F5}", country: "Japan" },
  kk: { flag: "\u{1F1F0}\u{1F1FF}", country: "Kazakhstan" },
  km: { flag: "\u{1F1F0}\u{1F1ED}", country: "Cambodia" },
  kn: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  ko: { flag: "\u{1F1F0}\u{1F1F7}", country: "South Korea" },
  ks: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  ku: { flag: GLOBE, country: "" },
  ky: { flag: "\u{1F1F0}\u{1F1EC}", country: "Kyrgyzstan" },
  lg: { flag: "\u{1F1FA}\u{1F1EC}", country: "Uganda" },
  ln: { flag: "\u{1F1E8}\u{1F1E9}", country: "DR Congo" },
  lt: { flag: "\u{1F1F1}\u{1F1F9}", country: "Lithuania" },
  luy: { flag: "\u{1F1F0}\u{1F1EA}", country: "Kenya" },
  mdh: { flag: "\u{1F1F5}\u{1F1ED}", country: "Philippines" },
  mg: { flag: "\u{1F1F2}\u{1F1EC}", country: "Madagascar" },
  mk: { flag: "\u{1F1F2}\u{1F1F0}", country: "North Macedonia" },
  ml: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  mos: { flag: "\u{1F1E7}\u{1F1EB}", country: "Burkina Faso" },
  mr: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  mrw: { flag: "\u{1F1F5}\u{1F1ED}", country: "Philippines" },
  ms: { flag: "\u{1F1F2}\u{1F1FE}", country: "Malaysia" },
  mt: { flag: "\u{1F1F2}\u{1F1F9}", country: "Malta" },
  ne: { flag: "\u{1F1F3}\u{1F1F5}", country: "Nepal" },
  nl: { flag: "\u{1F1F3}\u{1F1F1}", country: "Netherlands" },
  no: { flag: "\u{1F1F3}\u{1F1F4}", country: "Norway" },
  nqo: { flag: GLOBE, country: "" },
  ny: { flag: "\u{1F1F2}\u{1F1FC}", country: "Malawi" },
  om: { flag: "\u{1F1EA}\u{1F1F9}", country: "Ethiopia" },
  pa: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  pl: { flag: "\u{1F1F5}\u{1F1F1}", country: "Poland" },
  prs: { flag: "\u{1F1E6}\u{1F1EB}", country: "Afghanistan" },
  ps: { flag: "\u{1F1E6}\u{1F1EB}", country: "Afghanistan" },
  pt: { flag: "\u{1F1F5}\u{1F1F9}", country: "Portugal" },
  rn: { flag: "\u{1F1E7}\u{1F1EE}", country: "Burundi" },
  ro: { flag: "\u{1F1F7}\u{1F1F4}", country: "Romania" },
  ru: { flag: "\u{1F1F7}\u{1F1FA}", country: "Russia" },
  rw: { flag: "\u{1F1F7}\u{1F1FC}", country: "Rwanda" },
  sd: { flag: "\u{1F1F5}\u{1F1F0}", country: "Pakistan" },
  si: { flag: "\u{1F1F1}\u{1F1F0}", country: "Sri Lanka" },
  sn: { flag: "\u{1F1FF}\u{1F1FC}", country: "Zimbabwe" },
  so: { flag: "\u{1F1F8}\u{1F1F4}", country: "Somalia" },
  sq: { flag: "\u{1F1E6}\u{1F1F1}", country: "Albania" },
  sr: { flag: "\u{1F1F8}\u{1F1F7}", country: "Serbia" },
  sv: { flag: "\u{1F1F8}\u{1F1EA}", country: "Sweden" },
  sw: { flag: "\u{1F1F0}\u{1F1EA}", country: "Kenya" },
  ta: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  te: { flag: "\u{1F1EE}\u{1F1F3}", country: "India" },
  tg: { flag: "\u{1F1F9}\u{1F1EF}", country: "Tajikistan" },
  th: { flag: "\u{1F1F9}\u{1F1ED}", country: "Thailand" },
  tk: { flag: "\u{1F1F9}\u{1F1F2}", country: "Turkmenistan" },
  tl: { flag: "\u{1F1F5}\u{1F1ED}", country: "Philippines" },
  tr: { flag: "\u{1F1F9}\u{1F1F7}", country: "Turkey" },
  tt: { flag: "\u{1F1F7}\u{1F1FA}", country: "Russia" },
  tw: { flag: "\u{1F1EC}\u{1F1ED}", country: "Ghana" },
  ug: { flag: GLOBE, country: "" },
  uk: { flag: "\u{1F1FA}\u{1F1E6}", country: "Ukraine" },
  ur: { flag: "\u{1F1F5}\u{1F1F0}", country: "Pakistan" },
  uz: { flag: "\u{1F1FA}\u{1F1FF}", country: "Uzbekistan" },
  vi: { flag: "\u{1F1FB}\u{1F1F3}", country: "Vietnam" },
  yao: { flag: "\u{1F1F5}\u{1F1EC}", country: "Papua New Guinea" },
  yo: { flag: "\u{1F1F3}\u{1F1EC}", country: "Nigeria" },
  zgh: { flag: "\u{1F1F2}\u{1F1E6}", country: "Morocco" },
  zh: { flag: "\u{1F1E8}\u{1F1F3}", country: "China" },
  zu: { flag: "\u{1F1FF}\u{1F1E6}", country: "South Africa" },
});

const FLAG_FALLBACK: LanguageFlag = Object.freeze({ flag: GLOBE, country: "" });

/** Flag + country for a catalogue languageCode; globe fallback for unmapped codes. */
export function flagFor(languageCode: string): LanguageFlag {
  return LANGUAGE_FLAGS[languageCode] ?? FLAG_FALLBACK;
}

export function peekTranslationName(sourceId: string | undefined): string | null {
  if (!sourceId) return null;
  return TRANSLATION_CATALOGUE_BY_ID.get(sourceId)?.name ?? null;
}
