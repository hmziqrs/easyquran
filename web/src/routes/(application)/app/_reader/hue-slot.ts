/**
 * Landing surah-tile grammar (boards cycle the four hue slots by position):
 * soft fill + legible numeral, per §6 — the same pair the marketing index rows
 * render, so every reader index reads as the same product. Shared by the surah,
 * juz, and pages indexes.
 */
export const HUE_SOFT = {
  1: "var(--hue-1-soft)",
  2: "var(--hue-2-soft)",
  3: "var(--hue-3-soft)",
  4: "var(--hue-4-soft)",
} as const;

export const HUE_LEGIBLE = {
  1: "var(--hue-1-legible)",
  2: "var(--hue-2-legible)",
  3: "var(--hue-3-legible)",
  4: "var(--hue-4-legible)",
} as const;

export type HueSlot = keyof typeof HUE_SOFT;

export function hueSlotFor(index: number): HueSlot {
  // SAFETY: ((index-1) % 4) is 0–3 for any positive integer, so +1 is exactly 1–4.
  return (((index - 1) % 4) + 1) as HueSlot;
}
