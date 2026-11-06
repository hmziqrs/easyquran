/**
 * Branded share card (F13). Layout is pure and takes a `measure` function so it
 * can be tested without a canvas; `renderVerseCard` draws the laid-out lines on
 * a 2D canvas. Arabic is drawn with the reader's own faces (Amiri, Onest) and the
 * browser shapes it: text is passed verbatim, never re-encoded or split per glyph.
 */

export const CARD_WIDTH = 1080;
const PADDING = 96;
const ARABIC_SIZE = 58;
const ARABIC_LEADING = 1.9;
const LATIN_SIZE = 38;
const LATIN_LEADING = 1.5;
const REF_SIZE = 26;
const BRAND_SIZE = 34;
const SMALL_SIZE = 24;
const RULE_GAP = 40;
const SECTION_GAP = 56;

const ARABIC_FAMILY = "Amiri";
const LATIN_FAMILY = "Onest Variable";

const PALETTE = {
  background: "#FBF8F1",
  ink: "#1E2B25",
  muted: "#5F6B64",
  accent: "#0E6B5C",
  rule: "#D9D2BF",
} as const;

export interface CardVerse {
  readonly text: string;
  readonly ref: string;
}

export interface CardInput {
  readonly verses: readonly CardVerse[];
  readonly reference: boolean;
  /** Canonical link to print on the card, or null when the dialog left it off. */
  readonly link: string | null;
  readonly brand: string;
  readonly footer: string;
}

export interface CardLine {
  readonly text: string;
  readonly font: string;
  readonly y: number;
  readonly rtl: boolean;
  readonly color: string;
}

export interface CardLayout {
  readonly width: number;
  readonly height: number;
  readonly lines: readonly CardLine[];
}

/** Arabic-script detection: decides font and direction per line. */
export function isArabicScript(text: string): boolean {
  return /[؀-ۿݐ-ݿࢠ-ࣿﭐ-﷿ﹰ-﻿]/u.test(text);
}

/** Greedy word wrap. A single word wider than the line keeps its own line. */
export function wrapWords(
  words: readonly string[],
  maxWidth: number,
  measure: (line: string) => number,
): string[] {
  const lines: string[] = [];
  let current = "";
  for (const word of words) {
    if (current === "") {
      current = word;
      continue;
    }
    const candidate = `${current} ${word}`;
    if (measure(candidate) > maxWidth) {
      lines.push(current);
      current = word;
    } else {
      current = candidate;
    }
  }
  if (current !== "") lines.push(current);
  return lines;
}

interface BodyFont {
  readonly font: string;
  readonly leading: number;
}

function bodyFont(text: string): BodyFont {
  if (isArabicScript(text)) {
    return { font: `${ARABIC_SIZE}px ${ARABIC_FAMILY}`, leading: ARABIC_SIZE * ARABIC_LEADING };
  }
  return { font: `${LATIN_SIZE}px "${LATIN_FAMILY}"`, leading: LATIN_SIZE * LATIN_LEADING };
}

/**
 * Lays out the card top to bottom. `measure(font, text)` returns the advance
 * width of `text` in `font`; the canvas supplies it at render time.
 */
export function layoutCard(
  input: CardInput,
  measure: (font: string, text: string) => number,
): CardLayout {
  const maxWidth = CARD_WIDTH - PADDING * 2;
  const lines: CardLine[] = [];
  let y = PADDING;

  lines.push({
    text: input.brand,
    font: `600 ${BRAND_SIZE}px "${LATIN_FAMILY}"`,
    y,
    rtl: false,
    color: PALETTE.accent,
  });
  y += BRAND_SIZE + RULE_GAP;
  lines.push({ text: "", font: "", y, rtl: false, color: PALETTE.rule });
  y += SECTION_GAP;

  for (const verse of input.verses) {
    const { font, leading } = bodyFont(verse.text);
    const rtl = isArabicScript(verse.text);
    const words = verse.text.split(/\s+/u).filter((w) => w.length > 0);
    const wrapped = wrapWords(words, maxWidth, (line) => measure(font, line));
    for (const line of wrapped) {
      lines.push({ text: line, font, y, rtl, color: PALETTE.ink });
      y += leading;
    }
    if (input.reference) {
      y += 12;
      lines.push({
        text: verse.ref,
        font: `${REF_SIZE}px "${LATIN_FAMILY}"`,
        y,
        rtl: false,
        color: PALETTE.muted,
      });
      y += REF_SIZE * 1.5;
    }
    y += SECTION_GAP / 2;
  }

  y += RULE_GAP / 2;
  lines.push({ text: "", font: "", y, rtl: false, color: PALETTE.rule });
  y += SECTION_GAP;

  if (input.link !== null) {
    lines.push({
      text: input.link,
      font: `${SMALL_SIZE}px "${LATIN_FAMILY}"`,
      y,
      rtl: false,
      color: PALETTE.accent,
    });
    y += SMALL_SIZE * 1.6;
  }
  lines.push({
    text: input.footer,
    font: `${SMALL_SIZE}px "${LATIN_FAMILY}"`,
    y,
    rtl: isArabicScript(input.footer),
    color: PALETTE.muted,
  });
  y += SMALL_SIZE + PADDING;

  return { width: CARD_WIDTH, height: Math.ceil(y), lines };
}

/** Draws a laid-out card. Fonts must already be loaded (see renderVerseCard). */
function drawLayout(canvas: HTMLCanvasElement, layout: CardLayout): void {
  canvas.width = layout.width;
  canvas.height = layout.height;
  const ctx = canvas.getContext("2d");
  if (ctx === null) throw new Error("2d canvas unavailable");
  ctx.fillStyle = PALETTE.background;
  ctx.fillRect(0, 0, layout.width, layout.height);
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  for (const line of layout.lines) {
    if (line.text === "" && line.font === "") {
      ctx.fillStyle = line.color;
      ctx.fillRect(PADDING, line.y, layout.width - PADDING * 2, 2);
      continue;
    }
    ctx.font = line.font;
    ctx.fillStyle = line.color;
    ctx.direction = line.rtl ? "rtl" : "ltr";
    ctx.fillText(line.text, layout.width / 2, line.y);
  }
}

/** Renders the card to a PNG. Throws if the canvas cannot encode it. */
export async function renderVerseCard(input: CardInput): Promise<Blob> {
  const sample = input.verses.map((v) => v.text).join(" ");
  await Promise.all([
    document.fonts.load(`${ARABIC_SIZE}px ${ARABIC_FAMILY}`, sample).catch(() => undefined),
    document.fonts.load(`${LATIN_SIZE}px "${LATIN_FAMILY}"`, input.footer).catch(() => undefined),
  ]);
  const canvas = document.createElement("canvas");
  const probe = canvas.getContext("2d");
  if (probe === null) throw new Error("2d canvas unavailable");
  const layout = layoutCard(input, (font, text) => {
    probe.font = font;
    probe.direction = isArabicScript(text) ? "rtl" : "ltr";
    return probe.measureText(text).width;
  });
  drawLayout(canvas, layout);
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error("png encode failed"));
    }, "image/png");
  });
}
