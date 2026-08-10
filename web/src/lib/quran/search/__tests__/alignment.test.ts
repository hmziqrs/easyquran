import { describe, expect, it } from "vite-plus/test";
import { alignSearchText } from "../alignment.ts";

describe("alignSearchText", () => {
  it("identity path: same match/display text maps offsets directly", () => {
    const text = "بِسْمِ ٱللَّهِ";
    const aligned = alignSearchText(text, text);
    expect(aligned.matchNorm.length).toBe(aligned.matchDisplayStarts.length);
    expect(aligned.matchNorm.length).toBe(aligned.matchDisplayEnds.length);
    expect(aligned.matchDisplayStarts.every((s) => Number.isInteger(s) && s >= 0)).toBe(true);
    expect(aligned.matchDisplayEnds.every((e) => Number.isInteger(e) && e <= text.length)).toBe(
      true,
    );
  });

  it("identity path keeps an ornament as a searchable, highlightable token", () => {
    const text = "قول۞";
    const aligned = alignSearchText(text, text);
    expect(aligned.matchNorm).toContain("۞");
  });

  it("cross-script path: still aligns differing match/display texts", () => {
    const match = "بِسْمِ ٱللَّهِ";
    const display = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ";
    const aligned = alignSearchText(match, display);
    expect(aligned.matchDisplayStarts.length).toBe(aligned.matchNorm.length);
    expect(aligned.matchDisplayEnds.every((e) => e <= display.length)).toBe(true);
  });
});
