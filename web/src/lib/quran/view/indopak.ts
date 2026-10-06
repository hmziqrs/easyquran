export interface IndoPakEnding {
  readonly body: string;
  readonly lastWord: string;
  /** `lastWord` without trailing whitespace, and that whitespace (drawn zero-width). */
  readonly lastWordText: string;
  readonly lastWordSpace: string;
  readonly sign: string;
  readonly following: string;
  readonly ruku: boolean;
  readonly annotations: readonly IndoPakEndAnnotation[];
  readonly bodyParts: readonly IndoPakBodyPart[];
  readonly words: readonly IndoPakWord[];
}

export interface IndoPakEndAnnotation {
  readonly text: string;
  readonly offset: number;
  /** `text` split into the sign and its trailing source whitespace (e.g. U+2003), which is
      kept in the DOM but drawn zero-width so stacked signs stay together. */
  readonly mark: string;
  readonly space: string;
  /** Ink box of a zero-advance mark, in em; undefined for spacing glyphs. */
  readonly widthEm: number | undefined;
  /** Shift that puts the mark's ink at the start of its box, in em. */
  readonly indentEm: number | undefined;
}

export interface IndoPakBodyPart {
  readonly text: string;
  readonly offset: number;
  readonly annotations: readonly IndoPakEndAnnotation[];
}

/**
 * One atomic word box, as on Quran.com: lines break only between words, and pause signs
 * written after a word stay inside its box. `gap` is the source whitespace that follows.
 */
export interface IndoPakWord {
  readonly offset: number;
  readonly parts: readonly IndoPakBodyPart[];
  readonly gap: string;
  readonly stop: boolean;
}

const annotationInkEm = new Map<number, readonly [number, number]>([
  [0x0614, [-0.31, 0.258]],
  [0x0615, [0.247, 0.516]],
  [0x06d6, [-0.212, 0.255]],
  [0x06d7, [-0.18, 0.18]],
  [0x06d8, [-0.129, 0.085]],
  [0x06d9, [-0.157, 0.113]],
  [0x06da, [-0.146, 0.138]],
  [0x06db, [-0.054, 0.052]],
  [0x06dc, [-0.222, 0.217]],
  [0xe021, [-0.394, -0.199]],
  [0xe022, [-0.077, 0.172]],
]);

/** Quran.com's IndoPak stop signs plus this text's private pause/ruku signs. */
const STOP_SIGN = /[ؕؗۖ-ۜۢۥ۪۫-]/u;

function splitSpace(text: string): [string, string] {
  const space = /\s*$/u.exec(text)?.[0] ?? "";
  return [text.slice(0, text.length - space.length), space];
}

function splitMarkSpace(text: string): [string, string] {
  const space = /\s[\s\p{Cf}]*$/u.exec(text)?.[0] ?? "";
  return [text.slice(0, text.length - space.length), space];
}

function annotation(text: string, offset: number): IndoPakEndAnnotation {
  const [mark, space] = splitMarkSpace(text);
  const ink = annotationInkEm.get(text.charCodeAt(0));
  if (!ink) return { text, offset, mark, space, widthEm: undefined, indentEm: undefined };
  const [start, end] = ink;
  return { text, offset, mark, space, widthEm: round(end - start), indentEm: round(-start) };
}

function round(value: number): number {
  return Math.round(value * 1000) / 1000;
}

function endAnnotations(suffix: string, base = 0): IndoPakEndAnnotation[] {
  const parts = [...suffix.matchAll(/[-ؔؕۖ-ۜ][^-ؔؕۖ-ۜ]*/gu)];
  return parts.map((part) => annotation(part[0], base + part.index));
}

function bodyParts(text: string): IndoPakBodyPart[] {
  const clusters = text.matchAll(/[-ؔؕۖ-ۜ](?:[\p{M}\p{Cf}\s]*[-ؔؕۖ-ۜ])+[\p{M}\p{Cf}]*/gu);
  const parts: IndoPakBodyPart[] = [];
  let offset = 0;
  for (const cluster of clusters) {
    if (cluster.index > offset)
      parts.push({ text: text.slice(offset, cluster.index), offset, annotations: [] });
    parts.push({
      text: cluster[0],
      offset: cluster.index,
      annotations: endAnnotations(cluster[0]),
    });
    offset = cluster.index + cluster[0].length;
  }
  if (offset < text.length) parts.push({ text: text.slice(offset), offset, annotations: [] });
  return parts;
}

/** Splits after a U+200B-joined sign tail so the next word gets its own box (e.g. 17:7 qif). */
function joinedSegments(piece: string, offset: number): { text: string; offset: number }[] {
  const segments: { text: string; offset: number }[] = [];
  let start = 0;
  for (const match of piece.matchAll(/​[^\p{L}]*(?=\p{L})/gu)) {
    const end = match.index + match[0].length;
    if (match.index <= start) continue;
    segments.push({ text: piece.slice(start, end), offset: offset + start });
    start = end;
  }
  segments.push({ text: piece.slice(start), offset: offset + start });
  return segments;
}

interface WordDraft {
  offset: number;
  parts: IndoPakBodyPart[];
  gap: string;
}

function appendTo(word: WordDraft, part: IndoPakBodyPart) {
  const pending: IndoPakBodyPart[] = [];
  if (word.gap) {
    pending.push({ text: word.gap, offset: part.offset - word.gap.length, annotations: [] });
    word.gap = "";
  }
  pending.push(part);
  for (const item of pending) {
    const last = word.parts.at(-1);
    if (last && !last.annotations.length && !item.annotations.length) {
      word.parts[word.parts.length - 1] = { ...last, text: last.text + item.text };
    } else {
      word.parts.push(item);
    }
  }
}

const hasLetter = (text: string): boolean => /\p{L}/u.test(text);

function segmentWords(parts: readonly IndoPakBodyPart[]): WordDraft[] {
  const words: WordDraft[] = [];
  function attach(part: IndoPakBodyPart) {
    const current = words.at(-1);
    if (current) appendTo(current, part);
    else words.push({ offset: part.offset, parts: [part], gap: "" });
  }
  function piece(segment: string, offset: number) {
    const lead = /^[^\p{L}]*/u.exec(segment)?.[0] ?? "";
    if (lead.length === segment.length || !words.length) {
      attach({ text: segment, offset, annotations: [] });
      return;
    }
    if (lead) attach({ text: lead, offset, annotations: [] });
    const start = offset + lead.length;
    const letters = { text: segment.slice(lead.length), offset: start, annotations: [] };
    words.push({ offset: start, parts: [letters], gap: "" });
  }
  for (const part of parts) {
    if (part.annotations.length) {
      attach(part);
      continue;
    }
    for (const match of part.text.matchAll(/\s+|\S+/gu)) {
      const offset = part.offset + match.index;
      const current = words.at(-1);
      if (/^\s/u.test(match[0]) && current) {
        current.gap += match[0];
        continue;
      }
      for (const segment of joinedSegments(match[0], offset)) piece(segment.text, segment.offset);
    }
  }
  return words;
}

function toWord(word: WordDraft): IndoPakWord {
  return {
    offset: word.offset,
    parts: word.parts,
    gap: word.gap,
    stop: word.parts.some((part) => STOP_SIGN.test(part.text)),
  };
}

export function indopakEnding(text: string): IndoPakEnding {
  const ending = /([-][-\p{M}\p{Cf}\s]*)$/u.exec(text);
  const suffix = ending?.[1] ?? "";
  const beforeSuffix = text.slice(0, ending?.index ?? text.length);
  const drafts = segmentWords(bodyParts(beforeSuffix));
  const last = drafts.at(-1);
  const finalDraft =
    last && hasLetter(last.parts.map((part) => part.text).join("")) ? last : undefined;
  const finalStart = finalDraft?.offset ?? beforeSuffix.length;
  const body = beforeSuffix.slice(0, finalStart);
  const ruku = suffix.startsWith("");
  const sign = /^[-][\p{Cf}]*/u.exec(suffix)?.[0] ?? "";
  const lastWord = beforeSuffix.slice(finalStart);
  const [lastWordText, lastWordSpace] = splitSpace(lastWord);
  const words = finalDraft ? drafts.slice(0, -1) : drafts;
  return {
    body,
    lastWord,
    lastWordText,
    lastWordSpace,
    sign,
    following: suffix.slice(sign.length),
    ruku,
    annotations: endAnnotations(suffix),
    bodyParts: bodyParts(body),
    words: words.map((word) => toWord(word)),
  };
}
