import { highlightSegments } from "$lib/quran/search/highlights";
import { describe, expect, it } from "vite-plus/test";

describe("search highlight presentation", () => {
  it("partitions exact UTF-16 ranges without rewriting text", () => {
    const text = "قبل 😀 بعد";
    const segments = highlightSegments(text, [{ start: 4, end: 6 }]);
    expect(segments.map((segment) => segment.text).join("")).toBe(text);
    expect(segments.find((segment) => segment.highlighted)?.text).toBe("😀");
  });

  it("sorts, clamps, and merges untrusted overlapping ranges", () => {
    expect(
      highlightSegments("abcdef", [
        { start: 3, end: 99 },
        { start: 1, end: 4 },
        { start: -1, end: 2 },
        { start: 2.5, end: 4 },
      ]),
    ).toEqual([
      { start: 0, end: 1, text: "a", highlighted: false },
      { start: 1, end: 6, text: "bcdef", highlighted: true },
    ]);
  });
});
