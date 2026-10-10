import assert from "node:assert/strict";
import test from "node:test";

import {
  classifyMismatch,
  decodeMirrorVerses,
  describeDiff,
  diffSummary,
  formatCodePoint,
  mirrorBareText,
  ourBareText,
  pairBySlug,
  QURANCOM_PAIRS,
  type ParityBucket,
} from "../../web/src/routes/design/qurancom/compare.ts";

function classify(slug: string, mirrorRaw: string, oursRaw: string): ParityBucket {
  const pair = pairBySlug(slug);
  assert.ok(pair, `unknown pair ${slug}`);
  return classifyMismatch({
    pair,
    mirrorRaw,
    oursRaw,
    mirrorBare: mirrorBareText(pair, mirrorRaw),
    oursBare: ourBareText(pair, oursRaw),
  });
}

test("pair table covers the six quran.com scripts with unique slugs and sources", () => {
  assert.equal(QURANCOM_PAIRS.length, 6);
  const slugs = QURANCOM_PAIRS.map((pair) => pair.slug);
  assert.equal(new Set(slugs).size, slugs.length);
  for (const pair of QURANCOM_PAIRS) {
    assert.ok(pair.mirrorFile.endsWith(".json"), pair.slug);
    assert.ok(pair.mirrorField.startsWith("text_"), pair.slug);
    assert.ok(pair.ours.length > 0, pair.slug);
    for (const our of pair.ours) assert.ok(our.dbFile.endsWith(".sqlite"), our.dbFile);
  }
  assert.equal(pairBySlug("uthmani")?.ours.length, 2);
  assert.equal(pairBySlug("nope"), undefined);
});

test("diffSummary reports equality, first divergence, and full diff count", () => {
  const equal = diffSummary("abc", "abc");
  assert.equal(equal.equal, true);
  assert.equal(equal.diffCount, 0);
  assert.equal(equal.firstIndex, null);
  assert.equal(equal.mirrorCodePoint, null);

  const inline = diffSummary("aXc", "aYc");
  assert.equal(inline.equal, false);
  assert.equal(inline.diffCount, 1);
  assert.equal(inline.firstIndex, 1);
  assert.equal(inline.mirrorCodePoint, 0x58);
  assert.equal(inline.oursCodePoint, 0x59);

  const tail = diffSummary("ab", "abz");
  assert.equal(tail.firstIndex, 2);
  assert.equal(tail.diffCount, 1);
  assert.equal(tail.mirrorCodePoint, null);
  assert.equal(tail.oursCodePoint, 0x7a);

  const both = diffSummary("aXY", "aPQR");
  assert.equal(both.firstIndex, 1);
  assert.equal(both.diffCount, 3);
});

test("diffSummary aligns by codepoint, not UTF-16 unit", () => {
  const summary = diffSummary("a\u{10437}b", "a\u{10437}c");
  assert.equal(summary.firstIndex, 2);
  assert.equal(summary.mirrorCodePoint, 0x62);
  assert.equal(summary.oursCodePoint, 0x63);
});

test("formatCodePoint pads to four hex digits and names the end of text", () => {
  assert.equal(formatCodePoint(0x06ed), "U+06ED");
  assert.equal(formatCodePoint(0x200f), "U+200F");
  assert.equal(formatCodePoint(0xf519), "U+F519");
  assert.equal(formatCodePoint(0x1f600), "U+1F600");
  assert.equal(formatCodePoint(null), "end-of-text");
});

test("describeDiff names the first divergence", () => {
  const text = describeDiff(diffSummary("aX", "aY"));
  assert.match(text, /1 diffs/);
  assert.match(text, /index 1: U\+0058 vs U\+0059/);
  assert.equal(describeDiff(diffSummary("a", "a")), "equal");
});

test("classifyMismatch buckets real divergence shapes", () => {
  assert.equal(classify("indopak", "اَلْحَمْدُ لِلّٰهِ", "بِسۡمِ اللهِ اَلْحَمْدُ لِلّٰهِ"), "bismillah-packaging");
  assert.equal(classify("uthmani", "أَلِيمٌ بَعِيدٌ", "أَلِيمٌۢ بَعِيدٌ"), "annotation-marks");
  assert.equal(classify("uthmani", "أَلِيمٌ", "أَلِيمٌۭ"), "annotation-marks");
  assert.equal(classify("uthmani", "يَشَآءُ", "يَشَآءُ"), "maddah-encoding");
  assert.equal(classify("imlaei_simple", "انعمت", "أنعمت"), "hamza-encoding");
  assert.equal(classify("imlaei", "عَلِيمًا", "عَلِيما"), "tanween-encoding");
  assert.equal(classify("indopak", "كَلَٰمَ ۖ", "كَلَٰمَ ۚ"), "waqf-mark-difference");
  assert.equal(classify("indopak", "مَتَاعٌۙ‏", "مَتَاعٌۙ"), "direction-mark");
  assert.equal(classify("indopak", "جَارَةُ  ۖ", "جَارَةُ ۖ"), "spacing");
  assert.equal(classify("imlaei", "مَالِكِ", "مالك"), "vocalization");
  assert.equal(classify("uthmani", "ب", "ت"), "other");
});

test("classifyMismatch sees tajweed markup differences as markup, not text", () => {
  assert.equal(
    classify("uthmani_tajweed", "<tajweed class=ham_wasl>ٱ</tajweed>للَّهِ", "[h:1[ٱ]للَّهِ"),
    "tajweed-markup",
  );
  // Upstream 32:3 carries a malformed tag (`>ٮٰ</tajweed>`); the stray `>` is markup residue.
  assert.equal(classify("uthmani_tajweed", "فْتَرَ>ٮٰ</tajweed>هُ", "فْتَرَ[ٮٰ]هُ"), "tajweed-markup");
});

test("decodeMirrorVerses reads the API payload shape and skips malformed entries", () => {
  const payload = JSON.stringify({
    verses: [
      { verse_key: "1:1", text_x: "abc" },
      { verse_key: "1:2" },
      { text_x: "no key" },
      { verse_key: "1:3", text_x: 42 },
    ],
  });
  const verses = decodeMirrorVerses(payload, "text_x");
  assert.equal(verses.size, 1);
  assert.equal(verses.get("1:1"), "abc");
  assert.equal(decodeMirrorVerses("null", "text_x").size, 0);
});
