import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";

import { countBy, maxBy, sumBy } from "../../../web/node_modules/es-toolkit/dist/index.mjs";
import { indopakEnding } from "../../../web/src/lib/quran/view/indopak.ts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../../..");
const DATABASE = path.join(ROOT, "db/quran/arabic/quran-indopak.sqlite");
const PYTHON = path.join(ROOT, ".cache/indopak-venv/bin/python");
const OUTPUT = path.resolve(
  ROOT,
  process.env.INDOPAK_PHASEB_OUTPUT ?? ".cache/indopak-v4/2026-10-06-run1/phaseB",
);
const REPORT = path.join(HERE, "segmentation-report.json");
const SIZE_PX = 56;
const SIZE_ADJUST = 1.25;
const ORNAMENT_EM = 0.68;
const INLINE_GAP_EM = 0.16;
const STOP_MARGIN_EM = 0.27;
const FINAL_MARGIN_EM = 0.08;
const END_ROW_EM = 0.36;
const COLUMNS = { specimen_320: 320 - 2 * 1 - 2 * 16, reader_viewport_320: 320 - 2 * 24 };
const KNOWN_OVERFLOW = ["12:21", "19:17", "91:14"];

const DIAGNOSIS = {
  "R5.split-other":
    "A box boundary that is neither source whitespace, a moved letter-free prefix, a U+200B-joined split nor an inline-cluster split.",
  "R5.merged-words":
    "Source whitespace sits inside a box and letters follow it: two source words share one unbreakable box.",
};
const REVIEW = {
  "R5.verse-start-lead-cf":
    "Verse starts with invisible U+200B; it stays in the first box because nothing precedes it. No ink. Accepted.",
  "R5.sign-only-box":
    "76:17 starts U+200B U+0020 U+200B: an ink-free verse-initial box, the only letter-free box in the corpus. Accepted leading case.",
  "final.inline-private-sign":
    "A private pause sign inside the final word is drawn as plain text, not as an isolated inline sign.",
  "ink.multi-char-mark":
    "2:10 keeps U+06D9+U+06E2 and 7:137 keeps U+06D9+U+064E as attached combining-mark groups. Chromium 56px/640px original captures show their marks above the preceding word, with clear adjacent letters and no detached line start. Accepted geometry; no editorial sign-off implied.",
};
const HISTORY = {
  run1_failures: { "R5.lead-pause.word": 29, "R5.merged-words": 3 },
  run1_exceptions: {
    "R5.lead-cf.word": 22,
    "R5.lead-mark.word": 11,
    "R5.sign-only-box": 1,
    "R5.split-final-lead": 4,
    "ink.multi-char-mark": 1,
  },
  run1_overflow_explained:
    "Prior engine run overflow (56px / 286px): 19:17 and 91:14 were merged-word boxes (311.1 / 305.2px), now split; 12:21 is one whitespace-free source token (277.5px + 15.1px stop margin = 292.6px) and still overflows.",
  fix: "indopak.ts segmentWords: one rule for all words. A box starts at a letter; letter-free text (sign token, glued sign/mark prefix incl. U+200B-led, inline cluster) joins the previous box with the whitespace before it. Annotation marks draw inner whitespace (+ following Cf) zero-width (4:171).",
};

const QC_STOP = new Set([
  0x06d6, 0x06d7, 0x06d8, 0x06d9, 0x06da, 0x06db, 0x06dc, 0x06e2, 0x0615, 0x06ea, 0x06eb, 0x0617,
  0x06e5,
]);

const hex = (cp) => cp.toString(16).toUpperCase().padStart(4, "0");
const cps = (text) => Array.from(text, (character) => character.codePointAt(0));
const isPrivate = (cp) => cp >= 0xe000 && cp <= 0xf8ff;
const hasLetter = (text) => /\p{L}/u.test(text);

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function privatePauseCodes(mapping) {
  const codes = mapping.entries
    .filter((entry) => /pause|stop|end of/iu.test(entry.meaning))
    .map((entry) => Number.parseInt(entry.codepoint.slice(2), 16));
  const expected = [0xe01a, 0xe01b, 0xe01c, 0xe01e, 0xe01f, 0xe021, 0xe022];
  if (codes.join() !== expected.join())
    throw new Error(`pause set drifted: ${codes.map((cp) => hex(cp))}`);
  return new Set(codes);
}

function readVerses() {
  const url = pathToFileURL(DATABASE);
  url.search = "?mode=ro&immutable=1";
  const database = new DatabaseSync(url, { readOnly: true });
  try {
    return database
      .prepare('SELECT sura, aya, text FROM quran_text ORDER BY "index"')
      .all()
      .map((row) => ({
        key: `${row.sura}:${row.aya}`,
        ayah: Number(row.aya),
        text: String(row.text),
      }));
  } finally {
    database.close();
  }
}

const CATEGORIES = [
  "Lu",
  "Ll",
  "Lt",
  "Lm",
  "Lo",
  "Mn",
  "Mc",
  "Me",
  "Nd",
  "Nl",
  "No",
  "Pc",
  "Pd",
  "Ps",
  "Pe",
  "Pi",
  "Pf",
  "Po",
  "Sm",
  "Sc",
  "Sk",
  "So",
  "Zs",
  "Zl",
  "Zp",
  "Cc",
  "Cf",
  "Cs",
  "Co",
].map((name) => [name, new RegExp(`^\\p{gc=${name}}$`, "u")]);

function category(character) {
  return CATEGORIES.find(([, pattern]) => pattern.test(character))?.[0] ?? "Cn";
}

function contextSignature(chars, index) {
  const previous = chars[index - 1];
  const following = chars[index + 1];
  let start = index;
  while (start > 0 && category(chars[start - 1]).startsWith("M")) start -= 1;
  let end = index + 1;
  while (end < chars.length && category(chars[end]).startsWith("M")) end += 1;
  const list = (items) => items.map((character) => `${hex(character.codePointAt(0))},`).join("");
  const nearby = chars
    .slice(Math.max(0, index - 2), index + 3)
    .filter((character) => isPrivate(character.codePointAt(0)));
  return [
    previous === undefined ? "start" : category(previous),
    following === undefined ? "end" : category(following),
    list(chars.slice(start, index)),
    list(chars.slice(index + 1, end)),
    list(nearby),
  ].join("|");
}

function sketch(text, signs) {
  const out = [];
  for (const cp of cps(text)) {
    const character = String.fromCodePoint(cp);
    const attachedMark = /\p{M}/u.test(character) && out.at(-1) === "w";
    if (character === " ") out.push("_");
    else if (signs.has(cp) || isPrivate(cp) || /[\p{Cf}\p{Zs}]/u.test(character)) out.push(hex(cp));
    else if (/\p{L}/u.test(character)) {
      if (out.at(-1) !== "w") out.push("w");
    } else if (!attachedMark) out.push(hex(cp));
  }
  return out.join(" ");
}

const boxText = (word) => word.parts.map((part) => part.text).join("");

function domPartText(part) {
  if (!part.annotations.length) return part.text;
  return part.annotations.map((annotation) => annotation.mark + annotation.space).join("");
}

function leadPrefix(text) {
  return /^[^\p{L}]*/u.exec(text)?.[0] ?? "";
}

function prefixKind(prefix, signs) {
  if (!prefix) return "";
  const codes = cps(prefix);
  if (codes.some((cp) => signs.has(cp))) return "pause";
  if (codes.every((cp) => /\p{Cf}/u.test(String.fromCodePoint(cp)))) return "cf";
  return "mark";
}

function precededBy(text, offset) {
  if (offset === 0) return "verse-start";
  if (/\s/u.test(text[offset - 1])) return "whitespace";
  return "joined";
}

function analyse(verse, signs) {
  const { key, text } = verse;
  const ending = indopakEnding(text);
  const failures = [];
  const exceptions = [];
  const fail = (rule, offset, detail) => failures.push({ key, rule, offset, detail });
  const except = (rule, offset, detail) => exceptions.push({ key, rule, offset, detail });
  const suffix = ending.sign + ending.following;
  const finalOffset = ending.body.length;
  if (ending.body + ending.lastWord + suffix !== text) fail("R1.concat", 0, "body+lastWord+suffix");
  const wordsText = ending.words.map((word) => boxText(word) + word.gap).join("");
  if (wordsText !== ending.body) fail("R1.words", 0, "words+gaps != body");
  if (ending.lastWordText + ending.lastWordSpace !== ending.lastWord)
    fail("R1.lastWord", finalOffset, "");
  const endDom = ending.sign
    ? ending.annotations.map((annotation) => annotation.mark + annotation.space).join("")
    : "";
  const dom =
    ending.words.map((word) => word.parts.map(domPartText).join("") + word.gap).join("") +
    ending.lastWordText +
    ending.lastWordSpace +
    endDom;
  if (dom !== text) fail("R1.dom", 0, "rendered text nodes != DB string");
  if (ending.annotations.map((annotation) => annotation.text).join("") !== suffix)
    fail("R1.end-annotations", finalOffset, "annotations != sign+following");
  if (ending.annotations.length > 0 !== (ending.sign !== ""))
    fail("R1.end-sign", finalOffset, "annotations without sign or vice versa");
  if (ending.ruku !== ending.sign.startsWith("\uE022")) fail("R1.ruku", finalOffset, "");
  const suffixStart = text.length - suffix.length;
  for (const annotation of ending.annotations) {
    if (
      suffix.slice(annotation.offset, annotation.offset + annotation.text.length) !==
      annotation.text
    )
      fail("R1.offset", suffixStart + annotation.offset, "end annotation offset");
  }
  const unique = (values) => new Set(values).size === values.length;
  if (!unique(ending.words.map((word) => word.offset)))
    fail("keys.words", 0, "duplicate word offset");
  if (!unique(ending.annotations.map((annotation) => annotation.offset)))
    fail("keys.end", finalOffset, "duplicate end annotation offset");

  const boxes = [];
  const inline = [];
  ending.words.forEach((word, index) => {
    const textOfBox = boxText(word);
    if (!word.parts.length || !textOfBox) fail("box.empty", word.offset, "");
    if (word.offset !== word.parts[0]?.offset)
      fail("R1.offset", word.offset, "word offset != first part");
    if (text.slice(word.offset, word.offset + textOfBox.length) !== textOfBox)
      fail("R1.offset", word.offset, "box text not at its offset");
    if (!/^\s*$/u.test(word.gap))
      fail("R5.gap", word.offset + textOfBox.length, "non-whitespace gap");
    if (!unique(word.parts.map((part) => part.offset))) fail("keys.parts", word.offset, "");
    for (const part of word.parts) {
      if (text.slice(part.offset, part.offset + part.text.length) !== part.text)
        fail("R1.offset", part.offset, "part not at its offset");
      if (!part.annotations.length) continue;
      if (part.annotations.map((annotation) => annotation.text).join("") !== part.text)
        fail("R1.inline-annotations", part.offset, "annotations != part");
      if (!unique(part.annotations.map((annotation) => annotation.offset)))
        fail("keys.inline", part.offset, "");
      inline.push({ offset: part.offset, annotations: part.annotations, text: part.text });
    }
    for (const annotation of word.parts.flatMap((part) => part.annotations)) {
      if (annotation.mark + annotation.space !== annotation.text)
        fail("R1.annotation", word.offset, "mark+space");
    }
    const expectedStop = cps(textOfBox).some((cp) => signs.has(cp));
    if (word.stop !== expectedStop) fail("stop.flag", word.offset, `stop=${word.stop}`);
    boxes.push({ kind: "word", index, offset: word.offset, text: textOfBox, word });
  });
  boxes.push({
    kind: "final",
    index: ending.words.length,
    offset: finalOffset,
    text: ending.lastWordText,
  });
  for (const box of boxes) {
    const prefix = leadPrefix(box.text);
    const kind = prefixKind(prefix, signs);
    box.lead = kind;
    if (!hasLetter(box.text)) {
      const allowed = box.offset === 0;
      (allowed ? except : fail)("R5.sign-only-box", box.offset, sketch(box.text, signs));
    } else if (kind && box.offset > 0) {
      fail(
        `R5.box-lead-${kind}.${box.kind}`,
        box.offset,
        `${precededBy(text, box.offset)}: ${sketch(prefix, signs)}`,
      );
    } else if (kind) {
      except(`R5.verse-start-lead-${kind}`, box.offset, sketch(prefix, signs));
    }
    const tokens = box.text.split(/\s+/u).slice(1);
    if (tokens.some((token) => hasLetter(token)))
      fail("R5.merged-words", box.offset, sketch(box.text, signs));
    for (const match of box.text.matchAll(/\u200B[^\p{L}\s]*(?=\p{L})/gu)) {
      if (match.index > 0)
        fail("R5.zwsp-unsplit", box.offset + match.index, sketch(box.text, signs));
    }
  }
  const zwspSplits = [];
  const prefixMoves = [];
  const clusterSplits = [];
  let whitespaceSplits = 0;
  for (let index = 0; index + 1 < boxes.length; index += 1) {
    const current = boxes[index];
    const next = boxes[index + 1];
    if (current.word.gap) {
      whitespaceSplits += 1;
      continue;
    }
    const intoFinal = next.kind === "final";
    const glued = /\S*$/u.exec(text.slice(0, next.offset))?.[0] ?? "";
    if (glued && !hasLetter(glued)) {
      prefixMoves.push({
        offset: next.offset,
        signs: cps(glued).map((cp) => hex(cp)),
        final: intoFinal,
      });
      continue;
    }
    const tail = /\u200B[^\p{L}\s]*$/u.exec(current.text);
    if (tail) {
      zwspSplits.push({
        offset: next.offset,
        zwsp: current.offset + tail.index,
        signs: cps(tail[0])
          .slice(1)
          .map((cp) => hex(cp)),
        final: intoFinal,
      });
      continue;
    }
    const cluster = current.word.parts.at(-1);
    if (cluster?.annotations.length) {
      clusterSplits.push({
        offset: next.offset,
        signs: cluster.annotations.map((annotation) => hex(annotation.mark.codePointAt(0))),
        final: intoFinal,
      });
      continue;
    }
    fail(
      "R5.split-other",
      next.offset,
      `${sketch(current.text.slice(-6), signs)} | ${sketch(next.text.slice(0, 4), signs)}`,
    );
  }
  if (!hasLetter(ending.lastWordText)) fail("final.letter", finalOffset, "");
  if (/\s$/u.test(ending.lastWordText)) fail("final.trailing-space", finalOffset, "");
  if (!/^\s*$/u.test(ending.lastWordSpace)) fail("final.space", finalOffset, "");
  const innerSpace = /\s[^\p{L}]*$/u.exec(ending.lastWordText)?.[0];
  const finalInnerSpace = innerSpace === undefined ? undefined : sketch(innerSpace, signs);
  if (/[\uE01A-\uE01C\uE01E\uE01F]/u.test(ending.lastWordText))
    except("final.inline-private-sign", finalOffset, sketch(ending.lastWordText, signs));

  return {
    verse,
    ending,
    boxes,
    inline,
    zwspSplits,
    prefixMoves,
    clusterSplits,
    whitespaceSplits,
    finalInnerSpace,
    failures,
    exceptions,
  };
}

class ShapeRequests {
  runs = [];
  index = new Map();
  add(text, direction, language) {
    const id = `${direction}|${language}|${text}`;
    if (!this.index.has(id)) {
      this.index.set(id, this.runs.length);
      this.runs.push({ text, direction, language });
    }
    return this.index.get(id);
  }
}

const ornamentText = (ayah) =>
  `\u06DD${String(ayah).replace(/\d/gu, (digit) => "\u06F0\u06F1\u06F2\u06F3\u06F4\u06F5\u06F6\u06F7\u06F8\u06F9"[Number(digit)])}`;

function planWidths(analyses, requests) {
  for (const analysis of analyses) {
    for (const box of analysis.boxes) {
      if (box.kind === "final") {
        box.runs = [{ run: requests.add(box.text, "rtl", "ar") }];
        box.ornament = requests.add(ornamentText(analysis.verse.ayah), "ltr", "ur");
        continue;
      }
      box.runs = box.word.parts.map((part) => {
        if (!part.annotations.length) return { run: requests.add(part.text, "rtl", "ar") };
        return {
          items: part.annotations.map((annotation) => {
            if (annotation.widthEm !== undefined) return { em: annotation.widthEm };
            return { run: requests.add(annotation.mark, "ltr", "ar") };
          }),
        };
      });
    }
  }
}

function measure(analyses, shaped) {
  const em = (run) => (shaped.advances[run] / shaped.upem) * SIZE_ADJUST;
  for (const analysis of analyses) {
    for (const box of analysis.boxes) {
      let width = 0;
      for (const run of box.runs) {
        if (run.items) {
          width += sumBy(run.items, (item) => item.em ?? em(item.run));
          width += INLINE_GAP_EM * (run.items.length - 1);
        } else {
          width += em(run.run);
        }
      }
      if (box.kind === "final") width += em(box.ornament) * ORNAMENT_EM;
      box.em = Math.round(width * 1000) / 1000;
      box.px = Math.round(width * SIZE_PX * 10) / 10;
      let margin = 0;
      if (box.kind === "final") margin = FINAL_MARGIN_EM;
      else if (box.word.stop) margin = STOP_MARGIN_EM;
      box.marginPx = Math.round((width + margin) * SIZE_PX * 10) / 10;
    }
  }
}

function checkInk(analyses, shaped) {
  const failures = [];
  const exceptions = [];
  const leads = new Map();
  for (const analysis of analyses) {
    const lists = [
      ...analysis.inline.map((part) => ({
        base: part.offset,
        annotations: part.annotations,
        where: "inline",
      })),
      {
        base:
          analysis.verse.text.length - (analysis.ending.sign + analysis.ending.following).length,
        annotations: analysis.ending.annotations,
        where: "end",
      },
    ];
    for (const { base, annotations, where } of lists) {
      for (const annotation of annotations) {
        const [lead, ...rest] = cps(annotation.mark);
        const probe = shaped.probe[hex(lead)];
        const advance = sumBy(probe, (glyph) => glyph.advance);
        const offset = base + annotation.offset;
        const key = analysis.verse.key;
        const entry = leads.get(hex(lead)) ?? { advance, count: 0, where: new Set(), withBox: 0 };
        entry.count += 1;
        entry.where.add(where);
        if (annotation.widthEm !== undefined) entry.withBox += 1;
        leads.set(hex(lead), entry);
        const visibleRest = rest.filter((cp) => !/\p{Cf}/u.test(String.fromCodePoint(cp)));
        if (visibleRest.length)
          exceptions.push({
            key,
            rule: "ink.multi-char-mark",
            offset,
            detail: [lead, ...rest].map((cp) => hex(cp)).join(" "),
          });
        if (advance === 0 && annotation.widthEm === undefined)
          failures.push({ key, rule: "ink.missing-box", offset, detail: hex(lead) });
        if (advance !== 0 && annotation.widthEm !== undefined)
          failures.push({ key, rule: "ink.box-on-spacing-glyph", offset, detail: hex(lead) });
        if (annotation.widthEm === undefined || probe.length !== 1 || !probe[0].ink_x) continue;
        const [xMin, xMax] = probe[0].ink_x;
        const start = ((xMin + probe[0].x_offset) / shaped.upem) * SIZE_ADJUST;
        const end = ((xMax + probe[0].x_offset) / shaped.upem) * SIZE_ADJUST;
        if (
          Math.abs(end - start - annotation.widthEm) > 0.0015 ||
          Math.abs(-start - annotation.indentEm) > 0.0015
        )
          failures.push({
            key,
            rule: "ink.table-mismatch",
            offset,
            detail: `${hex(lead)} font [${start.toFixed(3)},${end.toFixed(3)}] vs width ${annotation.widthEm} indent ${annotation.indentEm}`,
          });
      }
    }
  }
  const summary = Object.fromEntries(
    [...leads].sort().map(([code, entry]) => [
      code,
      {
        advance: entry.advance,
        count: entry.count,
        with_ink_box: entry.withBox,
        where: [...entry.where].sort(),
      },
    ]),
  );
  return { failures, exceptions, summary };
}

function role(analysis, offset) {
  const { ending } = analysis;
  if (offset >= ending.body.length + ending.lastWord.length) return "end-stack";
  if (offset >= ending.body.length) return "final-word";
  const box = analysis.boxes.findLast(
    (candidate) => candidate.kind === "word" && candidate.offset <= offset,
  );
  const part = box.word.parts.findLast((candidate) => candidate.offset <= offset);
  const inBox = offset < box.offset + box.text.length;
  if (!inBox) return "gap";
  if (part.annotations.length) return "inline-cluster";
  if (offset < box.offset + leadPrefix(box.text).length) return "word-lead";
  return "word";
}

function contextCoverage(analyses, inventory) {
  const expected = new Map();
  for (const item of inventory.codepoints) {
    for (const context of item.contexts ?? [])
      expected.set(`${item.codepoint.slice(2)}|${context.signature}`, context);
  }
  const found = new Map();
  for (const analysis of analyses) {
    const chars = Array.from(analysis.verse.text);
    let offset = 0;
    chars.forEach((character, index) => {
      const cp = character.codePointAt(0);
      if (isPrivate(cp)) {
        const id = `${hex(cp)}|${contextSignature(chars, index)}`;
        const entry = found.get(id) ?? {
          count: 0,
          first: { key: analysis.verse.key, index },
          keys: new Set(),
          roles: {},
        };
        entry.count += 1;
        entry.keys.add(analysis.verse.key);
        const where = role(analysis, offset);
        entry.roles[where] = (entry.roles[where] ?? 0) + 1;
        found.set(id, entry);
      }
      offset += character.length;
    });
  }
  const mismatches = [];
  for (const [id, context] of expected) {
    const entry = found.get(id);
    if (!entry) mismatches.push({ id, problem: "class not found" });
    else if (
      entry.count !== context.count ||
      entry.first.key !== context.verse_key ||
      entry.first.index !== context.character_index
    )
      mismatches.push({
        id,
        problem: `count/first ${entry.count}@${entry.first.key}:${entry.first.index}`,
      });
  }
  for (const id of found.keys())
    if (!expected.has(id)) mismatches.push({ id, problem: "not in inventory" });
  const uncovered = new Set(expected.keys());
  const byVerse = new Map();
  for (const [id, entry] of found)
    for (const key of entry.keys) byVerse.set(key, [...(byVerse.get(key) ?? []), id]);
  const cover = [];
  while (uncovered.size) {
    const best = maxBy([...byVerse], ([, ids]) => ids.filter((id) => uncovered.has(id)).length);
    if (!best || !best[1].some((id) => uncovered.has(id))) break;
    cover.push(best[0]);
    for (const id of best[1]) uncovered.delete(id);
  }
  const classes = [...expected.keys()].map((id) => {
    const [code, ...signature] = id.split("|");
    const entry = found.get(id);
    return {
      code,
      signature: signature.join("|"),
      count: entry?.count ?? 0,
      first: entry?.first.key,
      capture: cover.find((key) => entry?.keys.has(key)),
      roles: entry?.roles ?? {},
    };
  });
  return {
    expected: expected.size,
    covered: classes.filter((entry) => entry.capture).length,
    mismatches,
    cover,
    classes,
  };
}

function specimenKeys(analyses, inventory, mapping) {
  const keys = new Set();
  for (const entry of mapping.entries)
    for (const context of entry.contexts) keys.add(context.verse_key);
  for (const item of inventory.codepoints)
    if (item.representative_verse_keys[0]) keys.add(item.representative_verse_keys[0]);
  for (const analysis of analyses)
    if (/\p{Co}/u.test(analysis.verse.text)) keys.add(analysis.verse.key);
  return keys;
}

function splitSummary(analyses, field) {
  const splits = analyses.flatMap((analysis) =>
    analysis[field].map((split) => ({ key: analysis.verse.key, ...split })),
  );
  return {
    count: splits.length,
    into_final_word: splits.filter((split) => split.final).length,
    by_signs: countBy(splits, (split) => split.signs.join(" ")),
    list: splits.map((split) => `${split.key}@${split.offset}:${split.signs.join(" ")}`),
  };
}

function histogram(values, edges) {
  return edges.map((edge, index) => {
    const upper = edges[index + 1];
    const label = upper === undefined ? `${edge}+` : `${edge}-${upper - 1}`;
    return [
      label,
      values.filter((value) => value >= edge && (upper === undefined || value < upper)).length,
    ];
  });
}

function main() {
  mkdirSync(OUTPUT, { recursive: true });
  const inventory = readJson(path.join(HERE, "inventory.json"));
  const mapping = readJson(path.join(HERE, "mapping.json"));
  const privatePause = privatePauseCodes(mapping);
  const signs = new Set([...QC_STOP, ...privatePause]);
  const verses = readVerses();
  if (verses.length !== inventory.verse_count) throw new Error(`verse count ${verses.length}`);
  const analyses = verses.map((verse) => analyse(verse, signs));

  const requests = new ShapeRequests();
  planWidths(analyses, requests);
  const leadCodes = new Set();
  for (const analysis of analyses) {
    for (const annotation of [
      ...analysis.ending.annotations,
      ...analysis.inline.flatMap((part) => part.annotations),
    ])
      leadCodes.add(hex(annotation.mark.codePointAt(0)));
  }
  const input = path.join(OUTPUT, "box-widths-input.json");
  const shapedPath = path.join(OUTPUT, "box-widths.json");
  writeFileSync(input, JSON.stringify({ runs: requests.runs, probe: [...leadCodes].sort() }));
  execFileSync(
    PYTHON,
    [path.join(HERE, "box_widths.py"), "--input", input, "--output", shapedPath],
    { stdio: "inherit" },
  );
  const shaped = readJson(shapedPath);
  measure(analyses, shaped);
  const ink = checkInk(analyses, shaped);

  const failures = [...analyses.flatMap((analysis) => analysis.failures), ...ink.failures];
  if (shaped.notdef.length)
    failures.push({
      key: "*",
      rule: "shape.notdef",
      offset: 0,
      detail: `${shaped.notdef.length} runs`,
    });
  const exceptions = [...analyses.flatMap((analysis) => analysis.exceptions), ...ink.exceptions];
  const coverage = contextCoverage(analyses, inventory);
  if (coverage.mismatches.length)
    failures.push({
      key: "*",
      rule: "context.inventory-mismatch",
      offset: 0,
      detail: JSON.stringify(coverage.mismatches.slice(0, 5)),
    });
  const specimens = specimenKeys(analyses, inventory, mapping);
  const allBoxes = analyses.flatMap((analysis) =>
    analysis.boxes.map((box) => ({ key: analysis.verse.key, box })),
  );
  const boxCounts = analyses.map((analysis) => analysis.boxes.length);
  const widest = [...allBoxes].sort((a, b) => b.box.px - a.box.px);
  const describe = ({ key, box }) => ({
    key,
    kind: box.kind,
    box: box.index,
    offset: box.offset,
    length: box.text.length,
    em: box.em,
    px_56: box.px,
    px_56_with_margin: box.marginPx,
    stop: box.kind === "word" && box.word.stop,
    in_prior_specimen_run: specimens.has(key),
    sketch: sketch(box.text, signs),
  });
  const overflow = Object.fromEntries(
    Object.entries(COLUMNS).map(([name, column]) => {
      const border = widest.filter(({ box }) => box.px > column);
      const withMargin = widest.filter(({ box }) => box.marginPx > column);
      const keys = (items) => [...new Set(items.map(({ key }) => key))].sort();
      const tested = (items) => keys(items).filter((key) => specimens.has(key));
      return [
        name,
        {
          column_px: column,
          border_box: keys(border),
          with_margin: keys(withMargin),
          border_box_in_prior_specimen_run: tested(border),
          with_margin_in_prior_specimen_run: tested(withMargin),
        },
      ];
    }),
  );
  const zwspSplits = analyses.flatMap((analysis) =>
    analysis.zwspSplits.map((split) => ({ key: analysis.verse.key, ...split })),
  );
  const stopBoxes = allBoxes.filter(({ box }) => box.kind === "word" && box.word.stop);
  const stopReason = ({ box }) => {
    const codes = cps(box.text);
    const qc = codes.some((cp) => QC_STOP.has(cp));
    const own = codes.some((cp) => privatePause.has(cp));
    if (qc && own) return "both";
    if (qc) return "quran_com_sign";
    return "private_pause_or_ruku";
  };
  const stopChars = {};
  for (const { box } of stopBoxes)
    for (const cp of new Set(cps(box.text)))
      if (signs.has(cp)) stopChars[hex(cp)] = (stopChars[hex(cp)] ?? 0) + 1;
  const stacks = analyses.filter((analysis) => analysis.ending.annotations.length > 0);
  const multi = stacks.filter((analysis) => analysis.ending.annotations.length > 1);
  const maxRows = Math.max(...stacks.map((analysis) => analysis.ending.annotations.length));
  const inlineClusters = analyses.flatMap((analysis) =>
    analysis.inline.map((part) => ({ key: analysis.verse.key, part })),
  );
  const finalStops = analyses.filter((analysis) =>
    cps(analysis.ending.lastWordText).some((cp) => signs.has(cp)),
  );

  const ruleCounts = (items) =>
    Object.fromEntries(Object.entries(countBy(items, (item) => item.rule)).sort());
  const compact = (item) => `${item.key}@${item.offset}${item.detail ? ` ${item.detail}` : ""}`;
  const byRule = (items) => {
    const grouped = {};
    for (const item of items) (grouped[item.rule] ??= []).push(compact(item));
    return grouped;
  };

  const report = {
    phase: "B",
    plan: "docs/indopak-v4-verification-plan.md#3-phase-b--renderer-logic-over-the-whole-corpus-new-tooling",
    command: "node scripts/fonts/indopak/segmentation_audit.mjs",
    database_id: "quran-indopak",
    database_open_mode: "ro&immutable=1",
    renderer: "web/src/lib/quran/view/indopak.ts indopakEnding (imported)",
    font: shaped.font,
    verses: analyses.length,
    history: HISTORY,
    assertions: {
      failures: failures.length,
      failures_by_rule: ruleCounts(failures),
      failures_list: byRule(failures),
      exceptions: exceptions.length,
      exceptions_by_rule: ruleCounts(exceptions),
      exceptions_list: byRule(exceptions),
      failure_diagnosis: Object.fromEntries(
        Object.keys(ruleCounts(failures)).map((rule) => [rule, DIAGNOSIS[rule] ?? "UNREVIEWED"]),
      ),
      exception_review: Object.fromEntries(
        Object.keys(ruleCounts(exceptions)).map((rule) => [rule, REVIEW[rule] ?? "UNREVIEWED"]),
      ),
    },
    stop_set: {
      quran_com: [...QC_STOP].map((cp) => hex(cp)).sort(),
      private_pause_ruku: [...privatePause].map((cp) => hex(cp)),
      source:
        "quran.com-frontend-next 74eb4e2 QuranWord.tsx INDO_PAK_STOP_SIGN_CHARS; mapping.json meanings",
    },
    distribution: {
      boxes_per_verse: {
        min: Math.min(...boxCounts),
        max: Math.max(...boxCounts),
        mean: Math.round((sumBy(boxCounts, (count) => count) / boxCounts.length) * 100) / 100,
        total: allBoxes.length,
        histogram: Object.fromEntries(histogram(boxCounts, [1, 5, 10, 20, 40, 80, 120])),
        largest: [...analyses]
          .sort((a, b) => b.boxes.length - a.boxes.length)
          .slice(0, 5)
          .map((analysis) => `${analysis.verse.key}:${analysis.boxes.length}`),
      },
      widest_box_per_verse_em_histogram: Object.fromEntries(
        histogram(
          analyses.map((analysis) => Math.floor(Math.max(...analysis.boxes.map((box) => box.em)))),
          [0, 1, 2, 3, 4, 5, 6],
        ),
      ),
      widest_boxes_56px: widest.slice(0, 25).map((item) => describe(item)),
      overflow_prediction_56px: { ...overflow, prior_run_before_fix: KNOWN_OVERFLOW },
      whitespace_splits: sumBy(analyses, (analysis) => analysis.whitespaceSplits),
      final_word_inner_space: countBy(
        analyses.filter((analysis) => analysis.finalInnerSpace),
        (analysis) => analysis.finalInnerSpace,
      ),
      zwsp_splits: {
        count: zwspSplits.length,
        into_final_word: zwspSplits.filter((split) => split.final).length,
        by_signs: countBy(zwspSplits, (split) => split.signs.join(" ") || "(none)"),
        list: zwspSplits.map(
          (split) =>
            `${split.key}@${split.offset}${split.signs.length ? `:${split.signs.join(" ")}` : ""}`,
        ),
      },
      prefix_moves: splitSummary(analyses, "prefixMoves"),
      cluster_splits: splitSummary(analyses, "clusterSplits"),
      stop_boxes: {
        count: stopBoxes.length,
        of_word_boxes: allBoxes.filter(({ box }) => box.kind === "word").length,
        by_reason: countBy(stopBoxes, stopReason),
        boxes_per_sign: Object.fromEntries(Object.entries(stopChars).sort()),
        final_words_holding_stop_sign_no_stop_margin: finalStops.length,
      },
      end_stacks: {
        verses_with_end_signs: stacks.length,
        ruku: analyses.filter((analysis) => analysis.ending.ruku).length,
        rows: countBy(stacks, (analysis) => analysis.ending.annotations.length),
        multi_sign: multi.length,
        max_rows: maxRows,
        max_height_em: Math.round(maxRows * END_ROW_EM * 100) / 100,
        multi_sign_keys: multi.map(
          (analysis) =>
            `${analysis.verse.key}:${analysis.ending.annotations.map((annotation) => hex(annotation.mark.codePointAt(0))).join("+")}`,
        ),
        last_word_space: countBy(
          analyses,
          (analysis) =>
            cps(analysis.ending.lastWordSpace)
              .map((cp) => hex(cp))
              .join(" ") || "(none)",
        ),
      },
      inline_clusters: {
        count: inlineClusters.length,
        signs: countBy(inlineClusters, ({ part }) => part.annotations.length),
        list: inlineClusters.map(
          ({ key, part }) =>
            `${key}@${part.offset}:${part.annotations.map((annotation) => hex(annotation.mark.codePointAt(0))).join("+")}`,
        ),
      },
      annotation_leads: ink.summary,
    },
    context_coverage: {
      classes: coverage.expected,
      covered: coverage.covered,
      inventory_mismatches: coverage.mismatches,
      phase_c_capture_keys: coverage.cover,
      phase_c_review_keys: [
        ...new Set([
          ...failures.map((item) => item.key),
          ...exceptions.map((item) => item.key),
          ...zwspSplits.map((split) => split.key),
          ...analyses
            .filter((analysis) => analysis.prefixMoves.length + analysis.clusterSplits.length)
            .map((analysis) => analysis.verse.key),
          ...multi.map((analysis) => analysis.verse.key),
          ...Object.values(overflow).flatMap((entry) => entry.with_margin),
        ]),
      ].filter((key) => key !== "*" && !coverage.cover.includes(key)),
      map: coverage.classes,
    },
    layout_model: {
      size_px: SIZE_PX,
      size_adjust: SIZE_ADJUST,
      ornament_em: ORNAMENT_EM,
      inline_gap_em: INLINE_GAP_EM,
      stop_margin_em: STOP_MARGIN_EM,
      final_margin_em: FINAL_MARGIN_EM,
      end_row_em: END_ROW_EM,
      note: "Box = inline-block border box: plain parts shaped RTL, inline sign items at widthEm or shaped advance plus gaps, final word + ornament; zero-width .indopak-space excluded.",
    },
  };
  writeFileSync(REPORT, `${JSON.stringify(report, null, 2)}\n`);

  const full = {
    ...report,
    failures,
    exceptions,
    zwsp_splits: zwspSplits,
    verses: analyses.map((analysis) => ({
      key: analysis.verse.key,
      boxes: analysis.boxes.map((box) => ({
        kind: box.kind,
        offset: box.offset,
        text: box.text,
        em: box.em,
        px_56: box.px,
        stop: box.kind === "word" && box.word.stop,
        gap: box.word?.gap,
        lead: box.lead,
      })),
      end: analysis.ending.annotations.map((annotation) => ({
        text: annotation.text,
        widthEm: annotation.widthEm,
        indentEm: annotation.indentEm,
      })),
      last_word_space: analysis.ending.lastWordSpace,
    })),
  };
  writeFileSync(path.join(OUTPUT, "segmentation-full.json"), JSON.stringify(full));
  console.log(
    `verses ${analyses.length}, boxes ${allBoxes.length}, failures ${failures.length}, exceptions ${exceptions.length}`,
  );
  console.log(`failures by rule ${JSON.stringify(report.assertions.failures_by_rule)}`);
  console.log(`exceptions by rule ${JSON.stringify(report.assertions.exceptions_by_rule)}`);
  console.log(
    `zwsp splits ${zwspSplits.length}, stop boxes ${stopBoxes.length}, multi end stacks ${multi.length} (max ${maxRows})`,
  );
  console.log(
    `context classes ${coverage.covered}/${coverage.expected}, mismatches ${coverage.mismatches.length}, capture verses ${coverage.cover.length}`,
  );
  console.log(`overflow ${JSON.stringify(overflow)}`);
  process.exitCode = failures.length ? 1 : 0;
}

main();
