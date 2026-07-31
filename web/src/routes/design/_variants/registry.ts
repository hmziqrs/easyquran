/* ════════════════════════════════════════════════════════════════════════
   registry.ts — the catalogue of design variants.

   Everything under /design is a review surface, not part of the product: the
   routes are noindex, absent from MARKETING_PAGES (so they never reach the
   sitemap, llms.txt or the .md/.txt variants), and nothing here is imported by
   the shipping pages. It exists so a direction can be picked by looking at
   three finished things side by side instead of describing them.

   Each variant is a self-contained component built ONLY from the design tokens
   (bg/fg/line/accent/pop), which is what makes the theme tweaker meaningful
   here — every variant re-skins live as the palette changes, so layout and
   palette can be judged together rather than one at a time.
   ════════════════════════════════════════════════════════════════════════ */

export type VariantId = "a" | "b" | "c";
export type VariantKind = "landing" | "reader";

export interface VariantDef {
  id: VariantId;
  /** short name used in the switcher and headings */
  name: string;
  /** the one-sentence thesis of the direction */
  pitch: string;
  /** what this direction trades away — the honest half of the pitch */
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

export const VARIANTS: Record<VariantKind, VariantDef[]> = {
  landing: LANDING_VARIANTS,
  reader: READER_VARIANTS,
};

export const isVariantId = (v: string): v is VariantId => v === "a" || v === "b" || v === "c";

export const variantDef = (kind: VariantKind, id: string): VariantDef | undefined =>
  VARIANTS[kind].find((v) => v.id === id);
