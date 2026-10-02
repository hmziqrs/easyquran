/**
 * Fine-grained reader spacing for the mix page. Each piece of an ayah row — the tools line,
 * the Arabic, every translation — owns its own padding instead of one row-wide padding, so
 * each can be tuned alone. Values ride the URL (short params, defaults omitted).
 */

export interface NumberTweakDef {
  readonly key: NumberTweakKey;
  readonly param: string;
  readonly label: string;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly unit: string;
  readonly group: TweakGroup;
}

export type TweakGroup = "Ayah row" | "Arabic" | "Translations" | "Lanes";

export type NumberTweakKey =
  | "rowX"
  | "toolsTop"
  | "rowBottom"
  | "arTop"
  | "arBottom"
  | "trSize"
  | "trLead"
  | "trY"
  | "trGap"
  | "labelSize"
  | "edge"
  | "inset"
  | "tint";

export type LabelPlacement = "inline" | "above" | "key";
export type LabelNames = "short" | "full";
export type OnOff = "on" | "off";

export interface Tweaks {
  readonly rowX: number;
  readonly toolsTop: number;
  readonly rowBottom: number;
  readonly arTop: number;
  readonly arBottom: number;
  readonly trSize: number;
  readonly trLead: number;
  readonly trY: number;
  readonly trGap: number;
  readonly labelSize: number;
  readonly edge: number;
  readonly inset: number;
  readonly tint: number;
  readonly label: LabelPlacement;
  readonly names: LabelNames;
  readonly tools: OnOff;
  readonly divider: OnOff;
  /** Custom dark grounds (`#rrggbb`), overriding the background preset when set. */
  readonly page: string;
  readonly reader: string;
}

export const DEFAULT_TWEAKS: Tweaks = {
  rowX: 36,
  toolsTop: 20,
  rowBottom: 26,
  arTop: 10,
  arBottom: 14,
  trSize: 17,
  trLead: 175,
  trY: 0,
  trGap: 10,
  labelSize: 13,
  edge: 3,
  inset: 14,
  tint: 0,
  label: "inline",
  names: "short",
  tools: "on",
  divider: "on",
  page: "",
  reader: "",
};

export const NUMBER_TWEAKS: readonly NumberTweakDef[] = [
  {
    key: "rowX",
    param: "rx",
    label: "Side padding",
    min: 12,
    max: 80,
    step: 2,
    unit: "px",
    group: "Ayah row",
  },
  {
    key: "toolsTop",
    param: "tt",
    label: "Space above tools",
    min: 0,
    max: 48,
    step: 2,
    unit: "px",
    group: "Ayah row",
  },
  {
    key: "rowBottom",
    param: "rb",
    label: "Space below ayah",
    min: 0,
    max: 80,
    step: 2,
    unit: "px",
    group: "Ayah row",
  },
  {
    key: "arTop",
    param: "at",
    label: "Space above Arabic",
    min: 0,
    max: 64,
    step: 2,
    unit: "px",
    group: "Arabic",
  },
  {
    key: "arBottom",
    param: "ab",
    label: "Space below Arabic",
    min: 0,
    max: 64,
    step: 2,
    unit: "px",
    group: "Arabic",
  },
  {
    key: "trSize",
    param: "ts",
    label: "Text size",
    min: 14,
    max: 24,
    step: 0.5,
    unit: "px",
    group: "Translations",
  },
  {
    key: "trLead",
    param: "tl",
    label: "Line height",
    min: 140,
    max: 220,
    step: 5,
    unit: "%",
    group: "Translations",
  },
  {
    key: "trY",
    param: "ty",
    label: "Padding per translation",
    min: 0,
    max: 24,
    step: 1,
    unit: "px",
    group: "Translations",
  },
  {
    key: "trGap",
    param: "tg",
    label: "Gap between translations",
    min: 0,
    max: 48,
    step: 2,
    unit: "px",
    group: "Translations",
  },
  {
    key: "labelSize",
    param: "ls",
    label: "Name size",
    min: 11,
    max: 17,
    step: 0.5,
    unit: "px",
    group: "Translations",
  },
  {
    key: "edge",
    param: "ew",
    label: "Edge width",
    min: 0,
    max: 8,
    step: 1,
    unit: "px",
    group: "Lanes",
  },
  {
    key: "inset",
    param: "ei",
    label: "Edge to text",
    min: 4,
    max: 36,
    step: 1,
    unit: "px",
    group: "Lanes",
  },
  {
    key: "tint",
    param: "tn",
    label: "Lane tint",
    min: 0,
    max: 16,
    step: 1,
    unit: "%",
    group: "Lanes",
  },
];

export const TWEAK_GROUPS: readonly TweakGroup[] = ["Ayah row", "Arabic", "Translations", "Lanes"];

const LABEL_PARAM = "lb";
const NAMES_PARAM = "nm";
const TOOLS_PARAM = "tb";
const DIVIDER_PARAM = "dv";
const PAGE_PARAM = "pg";
const READER_PARAM = "rd";
const HEX = /^#[0-9a-f]{6}$/i;

/** Every URL param the tweaks own, so a reset can drop them all. */
export const TWEAK_PARAMS: readonly string[] = [
  ...NUMBER_TWEAKS.map((def) => def.param),
  LABEL_PARAM,
  NAMES_PARAM,
  TOOLS_PARAM,
  DIVIDER_PARAM,
  PAGE_PARAM,
  READER_PARAM,
];

function readNumber(params: URLSearchParams, def: NumberTweakDef): number {
  const raw = params.get(def.param);
  const value = raw === null ? Number.NaN : Number(raw);
  if (!Number.isFinite(value)) return DEFAULT_TWEAKS[def.key];
  return Math.min(def.max, Math.max(def.min, value));
}

function readLabel(raw: string | null): LabelPlacement {
  if (raw === "above" || raw === "key") return raw;
  return "inline";
}

function readHex(raw: string | null): string {
  if (raw === null) return "";
  const value = raw.startsWith("#") ? raw : `#${raw}`;
  return HEX.test(value) ? value.toLowerCase() : "";
}

type MutableTweaks = { -readonly [K in keyof Tweaks]: Tweaks[K] };

export function readTweaks(params: URLSearchParams): Tweaks {
  const out: MutableTweaks = { ...DEFAULT_TWEAKS };
  for (const def of NUMBER_TWEAKS) out[def.key] = readNumber(params, def);
  out.label = readLabel(params.get(LABEL_PARAM));
  out.names = params.get(NAMES_PARAM) === "full" ? "full" : "short";
  out.tools = params.get(TOOLS_PARAM) === "off" ? "off" : "on";
  out.divider = params.get(DIVIDER_PARAM) === "off" ? "off" : "on";
  out.page = readHex(params.get(PAGE_PARAM));
  out.reader = readHex(params.get(READER_PARAM));
  return out;
}

/** `tweaks` with one numeric value replaced. */
export function withNumber(tweaks: Tweaks, key: NumberTweakKey, value: number): Tweaks {
  const next: MutableTweaks = { ...tweaks };
  next[key] = value;
  return next;
}

function setOrDrop(params: URLSearchParams, key: string, value: string, fallback: string): void {
  if (value === fallback) params.delete(key);
  else params.set(key, value);
}

/** Writes `tweaks` into `params`, leaving out every value that equals its default. */
export function writeTweaks(params: URLSearchParams, tweaks: Tweaks): void {
  for (const def of NUMBER_TWEAKS) {
    setOrDrop(params, def.param, String(tweaks[def.key]), String(DEFAULT_TWEAKS[def.key]));
  }
  setOrDrop(params, LABEL_PARAM, tweaks.label, DEFAULT_TWEAKS.label);
  setOrDrop(params, NAMES_PARAM, tweaks.names, DEFAULT_TWEAKS.names);
  setOrDrop(params, TOOLS_PARAM, tweaks.tools, DEFAULT_TWEAKS.tools);
  setOrDrop(params, DIVIDER_PARAM, tweaks.divider, DEFAULT_TWEAKS.divider);
  setOrDrop(params, PAGE_PARAM, tweaks.page.replace("#", ""), "");
  setOrDrop(params, READER_PARAM, tweaks.reader.replace("#", ""), "");
}

/** CSS custom properties the mix components read for spacing and type. */
export function tweakStyle(tweaks: Tweaks): string {
  return [
    `--row-x: ${tweaks.rowX}px`,
    `--tools-top: ${tweaks.toolsTop}px`,
    `--row-bottom: ${tweaks.rowBottom}px`,
    `--ar-top: ${tweaks.arTop}px`,
    `--ar-bottom: ${tweaks.arBottom}px`,
    `--reader-translation-size: ${tweaks.trSize}px`,
    `--tr-lead: ${tweaks.trLead / 100}`,
    `--tr-y: ${tweaks.trY}px`,
    `--tr-gap: ${tweaks.trGap}px`,
    `--label-size: ${tweaks.labelSize}px`,
    `--lane-edge: ${tweaks.edge}px`,
    `--lane-inset: ${tweaks.inset}px`,
    `--lane-tint: ${tweaks.tint}%`,
  ].join("; ");
}
