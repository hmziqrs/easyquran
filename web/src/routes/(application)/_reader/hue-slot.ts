/**
 * Card hue grammar (boards cycle the eight hue slots by position): soft fill +
 * legible numeral for the older index tiles; accent stroke + dim wash for the
 * surah cards. Shared by the surah, juz, and pages indexes.
 */
export const HUE_SOFT = {
  1: "var(--hue-1-soft)",
  2: "var(--hue-2-soft)",
  3: "var(--hue-3-soft)",
  4: "var(--hue-4-soft)",
  5: "var(--hue-5-soft)",
  6: "var(--hue-6-soft)",
  7: "var(--hue-7-soft)",
  8: "var(--hue-8-soft)",
} as const;

export const HUE_LEGIBLE = {
  1: "var(--hue-1-legible)",
  2: "var(--hue-2-legible)",
  3: "var(--hue-3-legible)",
  4: "var(--hue-4-legible)",
  5: "var(--hue-5-legible)",
  6: "var(--hue-6-legible)",
  7: "var(--hue-7-legible)",
  8: "var(--hue-8-legible)",
} as const;

/** Accent hue stroke for the round number medallion on surah cards. */
export const HUE_EDGE = {
  1: "var(--hue-1)",
  2: "var(--hue-2)",
  3: "var(--hue-3)",
  4: "var(--hue-4)",
  5: "var(--hue-5)",
  6: "var(--hue-6)",
  7: "var(--hue-7)",
  8: "var(--hue-8)",
} as const;

/** Dim transparent hue wash behind the medallion numeral. */
export const HUE_DIM = {
  1: "color-mix(in oklab, var(--hue-1) 30%, transparent)",
  2: "color-mix(in oklab, var(--hue-2) 30%, transparent)",
  3: "color-mix(in oklab, var(--hue-3) 30%, transparent)",
  4: "color-mix(in oklab, var(--hue-4) 30%, transparent)",
  5: "color-mix(in oklab, var(--hue-5) 30%, transparent)",
  6: "color-mix(in oklab, var(--hue-6) 30%, transparent)",
  7: "color-mix(in oklab, var(--hue-7) 30%, transparent)",
  8: "color-mix(in oklab, var(--hue-8) 30%, transparent)",
} as const;

export type HueSlot = keyof typeof HUE_SOFT;

export function hueSlotFor(index: number): HueSlot {
  // SAFETY: ((index-1) % 8) is 0–7 for any positive integer, so +1 is exactly 1–8.
  return (((index - 1) % 8) + 1) as HueSlot;
}
