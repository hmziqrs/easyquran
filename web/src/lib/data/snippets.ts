/**
 * Curated verse snippets (Feature 7, D06): Azkār, duas, and essential verses
 * addressed as *references only* — never copied Quran text (the DBs are
 * immutable; the reader renders text at request/prerender time). Editorial
 * content (titles, descriptions, kind labels) is bilingual in the data so the
 * localized page needs no per-entry message keys. Client-safe: pure data.
 */
export type SnippetKind = "essential" | "azkar" | "dua";

export interface SnippetRef {
  readonly surah: number;
  readonly ayahStart: number;
  readonly ayahEnd: number;
}

export interface SnippetText {
  readonly en: string;
  readonly ar: string;
}

export interface Snippet {
  readonly slug: string;
  readonly kind: SnippetKind;
  readonly title: SnippetText;
  readonly description: SnippetText;
  readonly refs: readonly SnippetRef[];
}

export const SNIPPET_KIND_LABELS = {
  essential: { en: "Essential verses", ar: "آيات أساسية" },
  azkar: { en: "Azkar", ar: "أذكار" },
  dua: { en: "Dua", ar: "دعاء" },
} satisfies Readonly<Record<SnippetKind, SnippetText>>;

/** /azkar index chrome — bilingual in the data like the entries, so the
 * static editorial index needs no per-page message keys. */
export const SNIPPET_INDEX_TITLE = {
  en: "Azkar & essential verses",
  ar: "أذكار وآيات مختارة",
} satisfies SnippetText;

export const SNIPPET_INDEX_DESCRIPTION = {
  en: "Curated verses and remembrances — Ayat al-Kursi, the closing of Al-Baqarah, and more, with any translation you read in.",
  ar: "آيات وأذكار مختارة — آية الكرسي وخواتيم البقرة وغيرها، مع الترجمة التي تقرأ بها.",
} satisfies SnippetText;

/** Display order of the /azkar index sections. */
export const SNIPPET_KIND_ORDER: readonly SnippetKind[] = ["essential", "azkar", "dua"];

export const SNIPPETS: readonly Snippet[] = [
  {
    slug: "ayat-al-kursi",
    kind: "essential",
    title: { en: "Ayat al-Kursi (The Throne Verse)", ar: "آية الكرسي" },
    description: {
      en: "The greatest verse of the Qur'an, recited for protection and remembrance.",
      ar: "أعظم آية في القرآن، تُتلى حرزًا وذكرًا.",
    },
    refs: [{ surah: 2, ayahStart: 255, ayahEnd: 255 }],
  },
  {
    slug: "baqarah-last-two",
    kind: "essential",
    title: { en: "The last two verses of Al-Baqarah", ar: "خواتيم سورة البقرة" },
    description: {
      en: "The closing verses of Al-Baqarah, among the most rewarding passages to recite at night.",
      ar: "الآيتان الأخيرتان من سورة البقرة، من أعظم ما يُتلى في الليل.",
    },
    refs: [{ surah: 2, ayahStart: 285, ayahEnd: 286 }],
  },
  {
    slug: "al-ikhlas",
    kind: "essential",
    title: { en: "Al-Ikhlas — Sincerity", ar: "سورة الإخلاص" },
    description: {
      en: "A short surah equal in reward to a third of the Qur'an.",
      ar: "سورة قصيرة تعدل ثلث القرآن أجرًا.",
    },
    refs: [{ surah: 112, ayahStart: 1, ayahEnd: 4 }],
  },
  {
    slug: "al-muawwidhatayn",
    kind: "azkar",
    title: { en: "Al-Mu'awwidhatayn — The two refuges", ar: "المعوذتان" },
    description: {
      en: "Al-Falaq and An-Nas, recited for protection morning and evening.",
      ar: "الفلق والناس، تُقرآن حرزًا صباحًا ومساءً.",
    },
    refs: [
      { surah: 113, ayahStart: 1, ayahEnd: 5 },
      { surah: 114, ayahStart: 1, ayahEnd: 6 },
    ],
  },
];

const SNIPPET_BY_SLUG: ReadonlyMap<string, Snippet> = new Map(
  SNIPPETS.map((snippet) => [snippet.slug, snippet]),
);

/** Kebab-case slug grammar shared by the route matcher copies (same shape as a surah slug). */
export const SNIPPET_SLUG_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u;

/** Curated lookup for the server-side parser: unknown slugs are not snippets. */
export function snippetForSlug(slug: string): Snippet | null {
  return SNIPPET_BY_SLUG.get(slug) ?? null;
}
