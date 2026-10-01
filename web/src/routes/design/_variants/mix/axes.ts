/**
 * Reader mix: each axis is one part of the reader chrome, each option one take on it.
 * Option "a" is always today's live look, so any axis can be compared against it.
 * Every combination is its own URL (`/design/mix?head=b&bar=c…`).
 */
export interface MixOption {
  readonly id: string;
  readonly name: string;
  readonly note: string;
}

export const HEAD_OPTIONS = [
  { id: "a", name: "Current", note: "Hue band behind the title, number tile on the left." },
  { id: "b", name: "Plain", note: "No band. Title and controls sit on the reading ground." },
  { id: "c", name: "Centered", note: "Arabic name leads, centred like a book's opening page." },
  { id: "d", name: "Edge", note: "No band; the surah's hue survives as a thin top edge and the number." },
  { id: "e", name: "Strip", note: "One compact row. The text starts sooner." },
] as const satisfies readonly MixOption[];

export const BAR_OPTIONS = [
  { id: "a", name: "Current", note: "Translate icon with a count badge." },
  { id: "b", name: "Count", note: "Icon, the word “Translations” and how many are on." },
  { id: "c", name: "Chips", note: "One chip per translation, language code first." },
  { id: "d", name: "Text", note: "No icon: “Translations” and the names, written out." },
] as const satisfies readonly MixOption[];

export const RESUME_OPTIONS = [
  { id: "a", name: "Current", note: "Blue band above the surah header." },
  { id: "b", name: "In bar", note: "A small “Resume 2:286” at the end of the sticky bar." },
  { id: "c", name: "Toast", note: "A dismissible note in the corner on arrival." },
  { id: "d", name: "In header", note: "One quiet line under the surah meta." },
  { id: "e", name: "Off", note: "Not in the reader; home and ⌘K still offer it." },
] as const satisfies readonly MixOption[];

export const LISTS_OPTIONS = [
  { id: "a", name: "Current", note: "Today's rows (with the stray bullet fixed)." },
  { id: "b", name: "Rows", note: "Number column, start and end of each range, page count." },
  { id: "c", name: "Tiles", note: "Juz as tiles; pages as a number grid grouped by juz." },
  { id: "d", name: "Arabic", note: "Each juz and page opens with its first words in Arabic." },
] as const satisfies readonly MixOption[];

export const STACK_OPTIONS = [
  { id: "a", name: "Current", note: "Rules between translations, inline translator label." },
  { id: "b", name: "Quiet", note: "No rules; label above each text; Urdu set in Naskh." },
  { id: "c", name: "Lanes", note: "Each translation owns a hue edge; its name runs inline in that hue (Tweak → Lanes to move it)." },
  { id: "d", name: "Columns", note: "Side by side on wide screens, names in a sticky header." },
  { id: "e", name: "Lead", note: "First translation leads; the rest fold behind “Compare”." },
] as const satisfies readonly MixOption[];

export const FRAME_OPTIONS = [
  { id: "a", name: "Card", note: "Today's frame: a bordered card a shade lighter than the page." },
  { id: "b", name: "Borderless", note: "A card told apart by its ground alone, no outline." },
  { id: "c", name: "Flat", note: "No card. The reading column sits on the page; only ayah rules remain." },
] as const satisfies readonly MixOption[];

export const BG_OPTIONS = [
  { id: "a", name: "Current", note: "Page 0.165, reader 0.19 — the grey the reader wears today." },
  { id: "b", name: "True black", note: "Pure #000 for page and reader." },
  { id: "c", name: "Ink", note: "One near-black (0.13) for page and reader." },
  { id: "d", name: "Soft black", note: "One slightly lifted black (0.155) for both." },
  { id: "e", name: "Black + ink", note: "Black page, ink (0.14) reading column." },
  { id: "f", name: "Ink + black", note: "Ink page, a blacker reading column (0.06)." },
  { id: "g", name: "Warm black", note: "Near-black with a trace of warmth (hue 70)." },
  { id: "h", name: "Cobalt black", note: "Near-black leaning to the cobalt accent (hue 262)." },
  { id: "i", name: "Green black", note: "Near-black leaning green (hue 162)." },
] as const satisfies readonly MixOption[];

export type HeadId = (typeof HEAD_OPTIONS)[number]["id"];
export type BarId = (typeof BAR_OPTIONS)[number]["id"];
export type ResumeId = (typeof RESUME_OPTIONS)[number]["id"];
export type ListsId = (typeof LISTS_OPTIONS)[number]["id"];
export type StackId = (typeof STACK_OPTIONS)[number]["id"];
export type FrameId = (typeof FRAME_OPTIONS)[number]["id"];
export type BgId = (typeof BG_OPTIONS)[number]["id"];

export interface MixState {
  readonly head: HeadId;
  readonly bar: BarId;
  readonly resume: ResumeId;
  readonly lists: ListsId;
  readonly stack: StackId;
  readonly frame: FrameId;
  readonly bg: BgId;
}

/** Dark-mode grounds per background preset. Light mode keeps the theme's own tokens. */
export interface BgColors {
  readonly page: string;
  readonly reader: string;
  readonly line: string;
}

export const BG_COLORS = {
  a: { page: "oklch(0.165 0 0)", reader: "oklch(0.19 0 0)", line: "oklch(0.275 0 0)" },
  b: { page: "oklch(0 0 0)", reader: "oklch(0 0 0)", line: "oklch(0.22 0 0)" },
  c: { page: "oklch(0.13 0 0)", reader: "oklch(0.13 0 0)", line: "oklch(0.24 0 0)" },
  d: { page: "oklch(0.155 0 0)", reader: "oklch(0.155 0 0)", line: "oklch(0.26 0 0)" },
  e: { page: "oklch(0 0 0)", reader: "oklch(0.14 0 0)", line: "oklch(0.24 0 0)" },
  f: { page: "oklch(0.15 0 0)", reader: "oklch(0.06 0 0)", line: "oklch(0.22 0 0)" },
  g: { page: "oklch(0.15 0.008 70)", reader: "oklch(0.15 0.008 70)", line: "oklch(0.26 0.012 70)" },
  h: { page: "oklch(0.15 0.014 262)", reader: "oklch(0.15 0.014 262)", line: "oklch(0.27 0.022 262)" },
  i: { page: "oklch(0.15 0.01 162)", reader: "oklch(0.15 0.01 162)", line: "oklch(0.26 0.016 162)" },
} as const satisfies { readonly [K in BgId]: BgColors };

export type MixAxisKey = keyof MixState;

export interface MixAxis {
  readonly key: MixAxisKey;
  readonly label: string;
  readonly options: readonly MixOption[];
}

export const MIX_AXES: readonly MixAxis[] = [
  { key: "head", label: "Surah header", options: HEAD_OPTIONS },
  { key: "bar", label: "Translation button", options: BAR_OPTIONS },
  { key: "resume", label: "Continue reading", options: RESUME_OPTIONS },
  { key: "lists", label: "Browse lists", options: LISTS_OPTIONS },
  { key: "stack", label: "Stacked translations", options: STACK_OPTIONS },
  { key: "frame", label: "Reader frame", options: FRAME_OPTIONS },
  { key: "bg", label: "Background", options: BG_OPTIONS },
];

export const CURRENT_MIX: MixState = {
  head: "a",
  bar: "a",
  resume: "a",
  lists: "a",
  stack: "a",
  frame: "a",
  bg: "a",
};
// The user's 2026-10-01 pick (strip, count, off, rows, lanes) plus a grey-free ground.
export const RECOMMENDED_MIX: MixState = {
  head: "e",
  bar: "b",
  resume: "e",
  lists: "b",
  stack: "c",
  frame: "c",
  bg: "c",
};

function pick<const T extends readonly MixOption[]>(
  options: T,
  raw: string | null,
  fallback: T[number]["id"],
): T[number]["id"] {
  const hit = options.find((option) => option.id === raw);
  return hit ? hit.id : fallback;
}

/** Unknown or missing params fall back to the recommended pick for that axis. */
export function readMix(params: URLSearchParams): MixState {
  return {
    head: pick(HEAD_OPTIONS, params.get("head"), RECOMMENDED_MIX.head),
    bar: pick(BAR_OPTIONS, params.get("bar"), RECOMMENDED_MIX.bar),
    resume: pick(RESUME_OPTIONS, params.get("resume"), RECOMMENDED_MIX.resume),
    lists: pick(LISTS_OPTIONS, params.get("lists"), RECOMMENDED_MIX.lists),
    stack: pick(STACK_OPTIONS, params.get("stack"), RECOMMENDED_MIX.stack),
    frame: pick(FRAME_OPTIONS, params.get("frame"), RECOMMENDED_MIX.frame),
    bg: pick(BG_OPTIONS, params.get("bg"), RECOMMENDED_MIX.bg),
  };
}

/** `?…` for `params` with every axis of `mix` written out (so the URL names the whole mix). */
export function mixSearch(params: URLSearchParams, mix: MixState): string {
  const next = new URLSearchParams(params);
  for (const axis of MIX_AXES) next.set(axis.key, mix[axis.key]);
  return `?${next.toString()}`;
}

/** `?…` for the current mix with one axis switched to `id`. Lists links also open the drawer. */
export function axisSearch(
  params: URLSearchParams,
  mix: MixState,
  key: MixAxisKey,
  id: string,
): string {
  const next = new URLSearchParams(params);
  for (const axis of MIX_AXES) next.set(axis.key, mix[axis.key]);
  next.set(key, id);
  if (key === "lists") next.set("drawer", "1");
  return `?${next.toString()}`;
}

/** `?…` for the current mix with a different translation set. */
export function translationsSearch(
  params: URLSearchParams,
  mix: MixState,
  ids: readonly string[],
): string {
  const next = new URLSearchParams(params);
  for (const axis of MIX_AXES) next.set(axis.key, mix[axis.key]);
  next.set("t", ids.join(","));
  return `?${next.toString()}`;
}
