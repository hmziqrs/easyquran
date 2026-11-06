import { describe, expect, it } from "vite-plus/test";

import { SHARE_SPAN_MAX, rangeProblem } from "../range";

describe("rangeProblem", () => {
  it("accepts an inclusive range inside the surah", () => {
    expect(rangeProblem(255, 286, 286)).toBeNull();
    expect(rangeProblem(1, 1, 7)).toBeNull();
  });

  it("rejects out-of-surah, reversed, zero and non-integer bounds", () => {
    expect(rangeProblem(0, 3, 7)).toBe("invalid");
    expect(rangeProblem(2, 8, 7)).toBe("invalid");
    expect(rangeProblem(5, 3, 7)).toBe("invalid");
    expect(rangeProblem(Number.NaN, 3, 7)).toBe("invalid");
    expect(rangeProblem(1.5, 3, 7)).toBe("invalid");
  });

  it("caps the span at the share limit, inclusive", () => {
    expect(SHARE_SPAN_MAX).toBe(50);
    expect(rangeProblem(1, 50, 200)).toBeNull();
    expect(rangeProblem(1, 51, 200)).toBe("too-long");
  });
});
