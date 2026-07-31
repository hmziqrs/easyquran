/* ════════════════════════════════════════════════════════════════════════
   derive.ts — turn three "seed" colours into a full token set.

   The theme tweaker lets a user pick just three colours: a background, an
   accent, and a pop. Everything else in the palette (the panel/inset/hover
   surface steps, the four foreground steps, the hairline steps, the soft/line
   accent washes, the accent's own foreground) is DERIVED here, so a custom
   theme stays internally consistent instead of needing a dozen pickers.

   Derivation is plain sRGB mixing rather than color-mix() in CSS, because the
   values are also written into a "Copy CSS" block that must paste cleanly into
   layout.css — and because we need the background's luminance to decide which
   DIRECTION to step (a dark seed steps toward white, a light seed toward
   black). The output is a flat record of CSS custom properties, applied as an
   inline style on <html> where it outranks every [data-theme]/[data-surface]
   rule in layout.css.
   ════════════════════════════════════════════════════════════════════════ */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** The three colours a user actually picks. Any subset may be set. */
export interface CustomSeeds {
  bg?: string;
  accent?: string;
  pop?: string;
}

/** #rgb / #rrggbb → {r,g,b}. Returns null on anything unparseable. */
export function parseHex(hex: string): Rgb | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  let h = m[1]!;
  if (h.length === 3) h = h[0]! + h[0]! + h[1]! + h[1]! + h[2]! + h[2]!;
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
  };
}

const clamp = (n: number): number => Math.max(0, Math.min(255, Math.round(n)));

export function toHex({ r, g, b }: Rgb): string {
  const p = (n: number) => clamp(n).toString(16).padStart(2, "0");
  return `#${p(r)}${p(g)}${p(b)}`;
}

/** Relative luminance (WCAG). Drives the light/dark decisions below. */
export function luminance({ r, g, b }: Rgb): number {
  const ch = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
}

/** True when text on this colour should be dark (i.e. the colour is light). */
export const isLight = (c: Rgb): boolean => luminance(c) > 0.45;

/** Linear mix of two colours; `t` is how much of `b` to take (0..1). */
function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return {
    r: a.r + (b.r - a.r) * t,
    g: a.g + (b.g - a.g) * t,
    b: a.b + (b.b - a.b) * t,
  };
}

const WHITE: Rgb = { r: 255, g: 255, b: 255 };
const BLACK: Rgb = { r: 0, g: 0, b: 0 };

/** Step a colour `t` toward white (positive) or black (negative). */
const shift = (c: Rgb, t: number): Rgb => (t >= 0 ? mix(c, WHITE, t) : mix(c, BLACK, -t));

const rgba = ({ r, g, b }: Rgb, a: number): string =>
  `rgba(${clamp(r)}, ${clamp(g)}, ${clamp(b)}, ${a})`;

/**
 * Surface + text ramp from a single background seed.
 *
 * On a DARK seed the surfaces climb toward white and the elevated nav/footer
 * sinks below the page; on a LIGHT seed they sink toward black and the
 * elevated bar rises. The foreground ramp always runs from near-maximum
 * contrast (--fg) down to the muted label step (--fg-4), and the hairlines are
 * translucent so they sit correctly over any of the surface steps.
 */
function backgroundTokens(seed: Rgb): Record<string, string> {
  const light = isLight(seed);
  const dir = light ? -1 : 1; // which way "up" is for surfaces
  const ink = light ? BLACK : WHITE; // where text is pulled from

  return {
    "--bg": toHex(seed),
    "--bg-1": toHex(shift(seed, dir * (light ? 0.0 : 0.055))),
    "--bg-2": toHex(shift(seed, dir * (light ? 0.035 : 0.1))),
    "--bg-3": toHex(shift(seed, dir * (light ? 0.075 : 0.14))),
    "--bg-elev": toHex(shift(seed, light ? 0.4 : -0.28)),

    "--line": rgba(ink, light ? 0.12 : 0.09),
    "--line-2": rgba(ink, light ? 0.18 : 0.14),
    "--line-3": rgba(ink, light ? 0.28 : 0.22),

    "--fg": toHex(mix(seed, ink, light ? 0.92 : 0.97)),
    "--fg-2": toHex(mix(seed, ink, light ? 0.76 : 0.8)),
    "--fg-3": toHex(mix(seed, ink, light ? 0.6 : 0.64)),
    "--fg-4": toHex(mix(seed, ink, light ? 0.45 : 0.48)),
  };
}

/** Accent wash + a readable foreground for text sitting ON the accent. */
function accentTokens(seed: Rgb): Record<string, string> {
  const onLight = isLight(seed);
  return {
    "--accent": toHex(seed),
    "--accent-soft": rgba(seed, 0.13),
    "--accent-line": rgba(seed, 0.32),
    // Text on an accent fill: a deeply darkened tint of the accent itself on a
    // light accent, white on a dark one — never flat #000, which reads harsh.
    "--accent-fg": onLight ? toHex(mix(seed, BLACK, 0.88)) : "#ffffff",
    "--ring": toHex(seed),
  };
}

function popTokens(seed: Rgb): Record<string, string> {
  return {
    "--pop": toHex(seed),
    "--pop-soft": rgba(seed, 0.13),
  };
}

/**
 * Full custom-token record for the seeds that are set. Unparseable or missing
 * seeds contribute nothing, so the preset layer in layout.css shows through
 * for that part of the palette.
 */
export function deriveTokens(seeds: CustomSeeds): Record<string, string> {
  const out: Record<string, string> = {};
  const bg = seeds.bg ? parseHex(seeds.bg) : null;
  const accent = seeds.accent ? parseHex(seeds.accent) : null;
  const pop = seeds.pop ? parseHex(seeds.pop) : null;
  if (bg) Object.assign(out, backgroundTokens(bg));
  if (accent) Object.assign(out, accentTokens(accent));
  if (pop) Object.assign(out, popTokens(pop));
  return out;
}

/** The derived tokens as a paste-ready CSS rule for layout.css. */
export function tokensToCss(tokens: Record<string, string>, selector = ":root"): string {
  const body = Object.entries(tokens)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join("\n");
  return `${selector} {\n${body}\n}`;
}
