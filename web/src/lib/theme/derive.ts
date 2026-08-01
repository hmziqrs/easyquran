export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export interface CustomSeeds {
  bg?: string;
  accent?: string;
  pop?: string;
}

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

export function luminance({ r, g, b }: Rgb): number {
  const ch = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
}

export const isLight = (c: Rgb): boolean => luminance(c) > 0.45;

function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return {
    r: a.r + (b.r - a.r) * t,
    g: a.g + (b.g - a.g) * t,
    b: a.b + (b.b - a.b) * t,
  };
}

const WHITE: Rgb = { r: 255, g: 255, b: 255 };
const BLACK: Rgb = { r: 0, g: 0, b: 0 };

const shift = (c: Rgb, t: number): Rgb => (t >= 0 ? mix(c, WHITE, t) : mix(c, BLACK, -t));

const rgba = ({ r, g, b }: Rgb, a: number): string =>
  `rgba(${clamp(r)}, ${clamp(g)}, ${clamp(b)}, ${a})`;

function backgroundTokens(seed: Rgb): Record<string, string> {
  const light = isLight(seed);
  const dir = light ? -1 : 1;
  const ink = light ? BLACK : WHITE;

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

function accentTokens(seed: Rgb): Record<string, string> {
  const onLight = isLight(seed);
  return {
    "--accent": toHex(seed),
    "--accent-soft": rgba(seed, 0.13),
    "--accent-line": rgba(seed, 0.32),
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

export function tokensToCss(tokens: Record<string, string>, selector = ":root"): string {
  const body = Object.entries(tokens)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join("\n");
  return `${selector} {\n${body}\n}`;
}
