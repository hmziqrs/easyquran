/**
 * Three complete reader design systems. Each is one set of decisions — ground, one accent
 * and its single job, one type scale, one radius, one spacing rhythm, one way to label —
 * applied to every piece of the page (nav, sticky bar, surah header, ayah rows, lanes).
 * Unlike /design/mix, nothing is mixed per axis: a system is picked whole.
 */
export type SystemId = "a" | "b" | "c";

export interface ReaderSystem {
  readonly id: SystemId;
  readonly name: string;
  readonly pitch: string;
  /** CSS custom properties the SystemReader reads; every value the page uses lives here. */
  readonly vars: string;
  readonly header: "centered" | "strip" | "editorial";
  readonly frame: "flat" | "card";
  readonly lanes: "above" | "panel" | "inline";
  readonly verseKey: "quiet" | "pill" | "margin";
  readonly rules: readonly { readonly label: string; readonly value: string }[];
  readonly swatches: readonly { readonly name: string; readonly value: string }[];
}

function vars(entries: readonly (readonly [string, string])[]): string {
  return entries.map(([k, v]) => `--sys-${k}: ${v}`).join("; ");
}

export const SYSTEMS: readonly ReaderSystem[] = [
  {
    id: "a",
    name: "Mushaf",
    pitch:
      "Warm black and one gold accent, the colours of a printed mushaf. Translations set in a book serif; the page is one centred column with no boxes, so the text carries the design.",
    header: "centered",
    frame: "flat",
    lanes: "above",
    verseKey: "quiet",
    vars: vars([
      ["page", "oklch(0.13 0.006 75)"],
      ["bar", "oklch(0.13 0.006 75 / 0.88)"],
      ["line", "oklch(0.27 0.02 80)"],
      ["text", "oklch(0.95 0.012 85)"],
      ["soft", "oklch(0.8 0.02 85)"],
      ["accent", "oklch(0.8 0.11 82)"],
      ["accent-fill", "oklch(0.8 0.11 82)"],
      ["on-accent", "oklch(0.18 0.02 80)"],
      ["accent-wash", "oklch(0.8 0.11 82 / 0.12)"],
      ["ui", '"Nunito Variable", ui-rounded, sans-serif'],
      ["ui-weight", "500"],
      ["read", '"Amiri", Georgia, serif'],
      ["read-size", "19px"],
      ["read-lead", "1.7"],
      ["label-size", "12.5px"],
      ["radius", "6px"],
      ["column", "760px"],
      ["ayah-gap", "40px"],
      ["lane-gap", "16px"],
    ]),
    rules: [
      {
        label: "Accent",
        value: "Gold, only for marks: ayah ornaments, translator names, the active control",
      },
      { label: "Type", value: "UI Nunito 500 · translations Amiri serif 19/1.7 · Urdu Naskh" },
      { label: "Scale", value: "12.5 · 14 · 19 · 24 · 44" },
      { label: "Shape", value: "6px radius, no cards, hairline rules" },
      { label: "Rhythm", value: "8px grid; 40 between ayahs, 16 between translations" },
      { label: "Labels", value: "Translator name above its text, in the accent" },
    ],
    swatches: [
      { name: "Page", value: "oklch(0.13 0.006 75)" },
      { name: "Text", value: "oklch(0.95 0.012 85)" },
      { name: "Gold", value: "oklch(0.8 0.11 82)" },
      { name: "Rule", value: "oklch(0.27 0.02 80)" },
    ],
  },
  {
    id: "b",
    name: "Ledger",
    pitch:
      "Today's reader made strict: true black, cobalt for everything you can press, one 10px radius and one type scale. Translations sit in soft cobalt panels, so each reads as its own block.",
    header: "strip",
    frame: "card",
    lanes: "panel",
    verseKey: "pill",
    vars: vars([
      ["page", "oklch(0 0 0)"],
      ["bar", "oklch(0 0 0 / 0.88)"],
      ["line", "oklch(0.26 0 0)"],
      ["text", "oklch(0.96 0 0)"],
      ["soft", "oklch(0.8 0 0)"],
      ["accent", "oklch(0.72 0.15 262)"],
      ["accent-fill", "oklch(0.52 0.2 262)"],
      ["on-accent", "oklch(1 0 0)"],
      ["accent-wash", "oklch(0.52 0.2 262 / 0.14)"],
      ["ui", '"Nunito Variable", ui-rounded, sans-serif'],
      ["ui-weight", "500"],
      ["read", '"Nunito Variable", ui-rounded, sans-serif'],
      ["read-size", "17px"],
      ["read-lead", "1.7"],
      ["label-size", "12.5px"],
      ["radius", "10px"],
      ["column", "1152px"],
      ["ayah-gap", "32px"],
      ["lane-gap", "8px"],
    ]),
    rules: [
      { label: "Accent", value: "Cobalt, only for what you can press or what is selected" },
      { label: "Type", value: "Nunito 500 throughout · translations 17/1.7 · Urdu Naskh" },
      { label: "Scale", value: "12.5 · 13.5 · 15 · 17 · 22" },
      { label: "Shape", value: "10px radius on every box: card, panels, buttons, chips" },
      { label: "Rhythm", value: "4px grid; 32 between ayahs, 8 between translation panels" },
      { label: "Labels", value: "Translator name above, small, in cobalt, inside its panel" },
    ],
    swatches: [
      { name: "Page", value: "oklch(0 0 0)" },
      { name: "Text", value: "oklch(0.96 0 0)" },
      { name: "Cobalt", value: "oklch(0.52 0.2 262)" },
      { name: "Panel", value: "oklch(0.52 0.2 262 / 0.14)" },
    ],
  },
  {
    id: "c",
    name: "Garden",
    pitch:
      "Airy and calm: true black, a single soft green, big type and lots of space. No boxes and no rules — ayahs are separated by air, and numbers hang in the margin like a printed book.",
    header: "editorial",
    frame: "flat",
    lanes: "inline",
    verseKey: "margin",
    vars: vars([
      ["page", "oklch(0 0 0)"],
      ["bar", "oklch(0 0 0 / 0.88)"],
      ["line", "oklch(0.24 0 0)"],
      ["text", "oklch(0.96 0 0)"],
      ["soft", "oklch(0.8 0 0)"],
      ["accent", "oklch(0.8 0.13 162)"],
      ["accent-fill", "oklch(0.8 0.13 162)"],
      ["on-accent", "oklch(0.2 0.03 162)"],
      ["accent-wash", "oklch(0.8 0.13 162 / 0.12)"],
      ["ui", '"Nunito Variable", ui-rounded, sans-serif'],
      ["ui-weight", "400"],
      ["read", '"Nunito Variable", ui-rounded, sans-serif'],
      ["read-size", "18px"],
      ["read-lead", "1.8"],
      ["label-size", "13px"],
      ["radius", "999px"],
      ["column", "820px"],
      ["ayah-gap", "56px"],
      ["lane-gap", "14px"],
    ]),
    rules: [
      { label: "Accent", value: "Soft green, only for ayah numbers and translator names" },
      { label: "Type", value: "Nunito 400 · translations 18/1.8 · Urdu Naskh" },
      { label: "Scale", value: "13 · 15 · 18 · 34" },
      { label: "Shape", value: "Pill controls, nothing else has a box" },
      { label: "Rhythm", value: "8px grid; 56 between ayahs, 14 between translations" },
      { label: "Labels", value: "Translator name runs into its text, in the accent" },
    ],
    swatches: [
      { name: "Page", value: "oklch(0 0 0)" },
      { name: "Text", value: "oklch(0.96 0 0)" },
      { name: "Green", value: "oklch(0.8 0.13 162)" },
      { name: "Soft", value: "oklch(0.8 0 0)" },
    ],
  },
];

export function systemById(id: string): ReaderSystem | undefined {
  return SYSTEMS.find((s) => s.id === id);
}
