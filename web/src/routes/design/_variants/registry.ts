export type VariantId = "a" | "b" | "c";
export type VariantKind = "landing" | "reader" | "system";

export interface VariantDef {
  id: VariantId;
  name: string;
  pitch: string;
  tradeoff: string;
}

export const LANDING_VARIANTS: VariantDef[] = [
  {
    id: "a",
    name: "Mihrab",
    pitch:
      "Reverent and centred. An arch motif, the opening ayah as the hero image, and very little else — the page behaves like a doorway.",
    tradeoff:
      "Lowest information density of the three; a visitor who wants feature facts has to scroll for them.",
  },
  {
    id: "b",
    name: "Spec",
    pitch:
      "Dense and typographic. Mono labels, hairline rules, numbers stated plainly, no cards — the page reads like a well-kept changelog.",
    tradeoff:
      "Reads as a developer tool. Warmth has to come entirely from the palette, so it leans hardest on getting the surface family right.",
  },
  {
    id: "c",
    name: "Editorial",
    pitch:
      "Split hero with a live mushaf specimen beside the headline, then numbered sections. Shows the product instead of describing it.",
    tradeoff:
      "The specimen panel is the whole first impression, so it has to survive every palette — and it dominates small screens.",
  },
];

export const READER_VARIANTS: VariantDef[] = [
  {
    id: "a",
    name: "Focus",
    pitch:
      "Full-bleed single column, no card, no borders. Chrome collapses into one slim floating bar so nothing frames the text.",
    tradeoff: "Per-ayah actions are further away; the page gives up its sense of structure.",
  },
  {
    id: "b",
    name: "Sidecar",
    pitch:
      "Two columns: a sticky ayah rail on the left for jumping around, verses on the right with numbers set in the margin, not inline.",
    tradeoff: "Needs real width — below ~900px it has to collapse back to one column.",
  },
  {
    id: "c",
    name: "Mushaf",
    pitch:
      "A framed page. Justified continuous script inside a ruled border with inline medallions, closest to the printed book.",
    tradeoff:
      "Continuous text makes per-ayah interaction (bookmark, note, copy) awkward by construction.",
  },
];

/** Whole reader design systems (_variants/system/systems.ts holds their tokens and rules). */
export const SYSTEM_VARIANTS: VariantDef[] = [
  {
    id: "a",
    name: "Mushaf",
    pitch: "Warm black, one gold accent, translations in a book serif, one centred column with no boxes.",
    tradeoff: "The narrowest column; wide screens keep a lot of empty margin.",
  },
  {
    id: "b",
    name: "Ledger",
    pitch: "Today's reader made strict: cobalt for what you press, one 10px radius, translations in soft panels.",
    tradeoff: "The most boxes of the three; it reads as an app more than a book.",
  },
  {
    id: "c",
    name: "Garden",
    pitch: "Airy: one soft green, big type, ayahs separated by space, numbers hanging in the margin.",
    tradeoff: "The most scrolling; fewer ayahs fit on a screen.",
  },
];

export const VARIANTS = {
  landing: LANDING_VARIANTS,
  reader: READER_VARIANTS,
  system: SYSTEM_VARIANTS,
} satisfies Record<VariantKind, VariantDef[]>;

export const isVariantId = (v: string): v is VariantId => v === "a" || v === "b" || v === "c";

export const variantDef = (kind: VariantKind, id: string): VariantDef | undefined =>
  VARIANTS[kind].find((v) => v.id === id);
