import { describe, expect, it } from "vite-plus/test";

import {
  decodeMirrorVerses,
  describeDiff,
  diffSummary,
  formatCodePoint,
  mirrorBareText,
  ourBareText,
  pairBySlug,
  stripMirrorMarkup,
} from "../compare.ts";

function pair(slug: string) {
  const found = pairBySlug(slug);
  if (found === undefined) throw new Error(`unknown pair ${slug}`);
  return found;
}

describe("diffSummary", () => {
  it("reports equality without a divergence", () => {
    const summary = diffSummary("نص", "نص");
    expect(summary.equal).toBe(true);
    expect(summary.diffCount).toBe(0);
    expect(summary.firstIndex).toBeNull();
  });

  it("reports the first divergence and codepoints", () => {
    const summary = diffSummary("بٌ ۖ", "بٌۭۖ");
    expect(summary.equal).toBe(false);
    expect(summary.diffCount).toBe(1);
    expect(summary.firstIndex).toBe(2);
    expect(formatCodePoint(summary.mirrorCodePoint)).toBe("U+0020");
    expect(formatCodePoint(summary.oursCodePoint)).toBe("U+06ED");
  });

  it("counts a tail-only divergence", () => {
    const summary = diffSummary("مَتَاعٌ ۟۠", "مَتَاعٌ ۟۠‏");
    expect(summary.firstIndex).toBe(11);
    expect(summary.mirrorCodePoint).toBeNull();
    expect(summary.oursCodePoint).toBe(0x200f);
    expect(summary.diffCount).toBe(1);
  });
});

describe("describeDiff", () => {
  it("formats the caption", () => {
    expect(describeDiff(diffSummary("ab", "ab"))).toBe("equal");
    expect(describeDiff(diffSummary("ab", "ac"))).toBe(
      "1 diffs — first divergence at index 1: U+0062 vs U+0063",
    );
  });
});

describe("bare text normalization", () => {
  it("passes stored text through for non-bare pairs", () => {
    const indopak = pair("indopak");
    expect(mirrorBareText(indopak, "بِسۡمِ اللهِ")).toBe("بِسۡمِ اللهِ");
    expect(ourBareText(indopak, "بِسۡمِ اللهِ")).toBe("بِسۡمِ اللهِ");
  });

  it("strips mirror HTML and our tajweed markup for the tajweed pair", () => {
    const tajweed = pair("uthmani_tajweed");
    const mirror = "<tajweed class=ham_wasl>ٱ</tajweed>للَّهِ <span class=end>١</span>";
    expect(stripMirrorMarkup(mirror)).toBe("ٱللَّهِ");
    expect(mirrorBareText(tajweed, mirror)).toBe("ٱللَّهِ");
    expect(ourBareText(tajweed, "[h:1[ٱ]للَّهِ")).toBe("ٱللَّهِ");
    expect(ourBareText(tajweed, "ٱفْتَرَ[ٮٰ]هُ")).toBe("ٱفْتَرَٮٰهُ");
  });
});

describe("decodeMirrorVerses", () => {
  it("reads verse_key plus the requested text field", () => {
    const payload = JSON.stringify({
      verses: [
        { verse_key: "1:1", text_uthmani: "a" },
        { verse_key: "1:2", text_uthmani: "b" },
      ],
    });
    const verses = decodeMirrorVerses(payload, "text_uthmani");
    expect(verses.get("1:1")).toBe("a");
    expect(verses.get("1:2")).toBe("b");
    expect(decodeMirrorVerses("{}", "text_uthmani").size).toBe(0);
  });
});
