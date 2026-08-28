import { clamp } from "es-toolkit";

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
    r: Number.parseInt(h.slice(0, 2), 16),
    g: Number.parseInt(h.slice(2, 4), 16),
    b: Number.parseInt(h.slice(4, 6), 16),
  };
}

const toChannel = (n: number): number => clamp(Math.round(n), 0, 255);

export function toHex({ r, g, b }: Rgb): string {
  const p = (n: number) => toChannel(n).toString(16).padStart(2, "0");
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

/* ── oklch support (plan 07 contrast gate; plan 01 neutral ground) ─────────────────────────
   The token contract migrates from hex to `oklch(L C H)` values. WCAG contrast still needs
   an sRGB relative luminance, so oklch is converted to sRGB via the OKLab matrices
   (Björn Ottosson's reference implementation) and fed through the same `luminance()` as
   hex. Out-of-gamut colors clamp to [0,255] — sufficient for contrast measurement, where
   the clamped extreme is always at least as far from the opposite extreme. */

export interface Oklch {
  l: number;
  c: number;
  h: number;
}

// Hue unit → degrees (CSS Color 4). `none` in any channel means zero.
function hueUnitFactor(unit: string): number {
  if (unit === "rad") return 180 / Math.PI;
  if (unit === "grad") return 0.9;
  if (unit === "turn") return 360;
  return 1; // deg
}

// `none` is a valid component for any channel (CSS Color 4) and means zero.
const NUMBER = "(none|[0-9]+(?:\\.[0-9]+)?(?:e-?[0-9]+)?%?)";
const OKLCH_RE = new RegExp(
  `^oklch\\(\\s*${NUMBER}\\s+${NUMBER}\\s+(none|[0-9]+(?:\\.[0-9]+)?(?:deg|rad|grad|turn)?)\\s*\\)$`,
  "i",
);

function parseComponent(raw: string, scalePercent: number): number {
  if (raw === "none") return 0;
  if (raw.endsWith("%")) return (Number.parseFloat(raw) / 100) * scalePercent;
  return Number.parseFloat(raw);
}

/** Parses `oklch(0.52 0.21 262)` / `oklch(52% 0.21 262deg)`. Rejects alpha (`/ …`) rather
 *  than guessing how a translucent token composites — contrast needs opaque pairs. */
export function parseOklch(value: string): Oklch | null {
  const m = OKLCH_RE.exec(value.trim());
  if (!m) return null;
  const l = parseComponent(m[1]!, 1);
  const c = parseComponent(m[2]!, 0.4);
  const hRaw = m[3]!;
  let h = 0;
  if (hRaw !== "none") {
    const unit = /^([0-9.]+)(deg|rad|grad|turn)?$/i.exec(hRaw);
    if (!unit) return null;
    h = Number.parseFloat(unit[1]!) * hueUnitFactor((unit[2] ?? "deg").toLowerCase());
  }
  if (!Number.isFinite(l) || !Number.isFinite(c) || !Number.isFinite(h)) return null;
  return { l, c, h };
}

// OKLab → LMS (the cubic-root domain), then LMS → linear sRGB. Coefficients are the
// published constants; they must never be rounded further or dark-greys drift.
const rgbFromOklab = (l: number, a: number, b: number): Rgb => {
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.291485548 * b;
  const lr = l_ * l_ * l_;
  const lg = m_ * m_ * m_;
  const lb = s_ * s_ * s_;
  return {
    r: 4.0767416621 * lr - 3.3077115913 * lg + 0.2309699292 * lb,
    g: -1.2684380046 * lr + 2.6097574011 * lg - 0.3413193965 * lb,
    b: -0.0041960863 * lr - 0.7034186147 * lg + 1.707614701 * lb,
  };
};

const encodeChannel = (linear: number): number => {
  const v = linear <= 0.0031308 ? linear * 12.92 : 1.055 * linear ** (1 / 2.4) - 0.055;
  return clamp(Math.round(v * 255), 0, 255);
};

/** oklch → sRGB (0–255, clamped to gamut). Chroma 0 makes every axis identical. */
export function oklchToRgb({ l, c, h }: Oklch): Rgb {
  const rad = (h * Math.PI) / 180;
  const lab = rgbFromOklab(l, c * Math.cos(rad), c * Math.sin(rad));
  return { r: encodeChannel(lab.r), g: encodeChannel(lab.g), b: encodeChannel(lab.b) };
}

/** Hex or oklch token value → Rgb. Returns null for anything else rather than guessing. */
export function parseColor(value: string): Rgb | null {
  const hex = parseHex(value);
  if (hex) return hex;
  const ok = parseOklch(value);
  if (ok) return oklchToRgb(ok);
  return null;
}

/** WCAG contrast ratio: (lighter + 0.05) / (darker + 0.05). >= 4.5 passes AA text. */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

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
  `rgba(${toChannel(r)}, ${toChannel(g)}, ${toChannel(b)}, ${a})`;

// A custom background seed overrides the §4 semantic contract's background/surface/foreground half
// (docs/design-system.md §4) plus the legacy --bg/--line/--fg ramp it replaced.
function backgroundTokens(seed: Rgb) {
  const light = isLight(seed);
  const dir = light ? -1 : 1;
  const ink = light ? BLACK : WHITE;

  const subtle = shift(seed, dir * (light ? 0.03 : 0.03));
  const surface = shift(seed, light ? 0.05 : 0.055);
  const raised = shift(seed, light ? 0.1 : 0.09);
  const hover = shift(seed, dir * (light ? 0.055 : 0.1));

  return {
    // §4 contract
    "--background": toHex(seed),
    "--background-subtle": toHex(subtle),
    "--surface": toHex(surface),
    "--surface-raised": toHex(raised),
    "--surface-hover": toHex(hover),
    "--foreground": toHex(mix(seed, ink, light ? 0.92 : 0.97)),
    "--foreground-secondary": toHex(mix(seed, ink, light ? 0.76 : 0.8)),
    "--muted": toHex(mix(seed, ink, light ? 0.6 : 0.64)),
    "--border": rgba(ink, light ? 0.12 : 0.09),
    "--border-strong": rgba(ink, light ? 0.18 : 0.14),
    "--reader-background": toHex(light ? raised : shift(seed, -0.2)),

    // legacy ramp (aliased consumers)
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

// A custom accent seed overrides the interactive (primary) family plus the legacy --accent ramp.
function accentTokens(seed: Rgb) {
  const onLight = isLight(seed);
  return {
    // §4 contract
    "--primary": toHex(seed),
    "--primary-hover": toHex(shift(seed, onLight ? -0.08 : 0.08)),
    "--primary-foreground": onLight ? toHex(mix(seed, BLACK, 0.88)) : "#ffffff",
    "--primary-soft": rgba(seed, onLight ? 0.12 : 0.16),
    "--focus-ring": toHex(seed),

    // legacy ramp
    "--accent": toHex(seed),
    "--accent-soft": rgba(seed, 0.13),
    "--accent-line": rgba(seed, 0.32),
    "--accent-fg": onLight ? toHex(mix(seed, BLACK, 0.88)) : "#ffffff",
    "--ring": toHex(seed),
  };
}

// A custom pop seed overrides the editorial accent (gold family) plus the legacy --pop pair.
// When an interactive accent seed is also present it keeps ownership of --primary/--focus-ring;
// the editorial `--accent` always follows the pop seed per the §4 contract.
function popTokens(seed: Rgb) {
  return {
    // §4 contract
    "--accent": toHex(seed),
    "--accent-strong": toHex(seed),
    "--accent-soft": rgba(seed, 0.13),

    // legacy pair
    "--pop": toHex(seed),
    "--pop-soft": rgba(seed, 0.13),
  };
}

export function deriveTokens(seeds: CustomSeeds) {
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
