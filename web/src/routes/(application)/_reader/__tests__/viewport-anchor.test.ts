import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { captureViewportAnchor, restoreViewportAnchor } from "../viewport-anchor";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("viewport anchors", () => {
  it("captures flat ayah row and restores same text point after reflow", () => {
    vi.stubGlobal("innerHeight", 800);
    vi.stubGlobal("scrollY", 1200);
    const root = document.createElement("ol");
    root.innerHTML =
      '<li data-local-page="3" data-verse-key="2:19"><span class="verse-text">text</span></li>';
    const row = root.querySelector("li")!;
    const text = root.querySelector<HTMLElement>(".verse-text")!;
    vi.spyOn(row, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 180, 600, 400));
    const measure = vi
      .spyOn(text, "getBoundingClientRect")
      .mockReturnValue(new DOMRect(0, 200, 600, 120));
    const anchor = captureViewportAnchor(root);
    expect(anchor).toEqual({
      kind: "verse",
      localPage: 3,
      verseKey: "2:19",
      viewportPoint: 260,
      ratio: 0.5,
    });
    measure.mockReturnValue(new DOMRect(0, 250, 600, 200));
    const scroll = vi.spyOn(window, "scrollTo").mockImplementation(() => {});
    expect(anchor && restoreViewportAnchor(root, anchor)).toBe(true);
    expect(scroll).toHaveBeenCalledWith(0, 1290);
  });

  it("finds inline ayah within reading page and preserves point below viewport marker", () => {
    vi.stubGlobal("innerHeight", 800);
    const root = document.createElement("div");
    root.innerHTML =
      '<section data-local-page="1" data-page-rendered><span data-verse-key="2:1"><span class="verse-text">text</span></span></section>';
    vi.spyOn(root.querySelector("section")!, "getBoundingClientRect").mockReturnValue(
      new DOMRect(0, 300, 600, 200),
    );
    vi.spyOn(root.querySelector(".verse-text")!, "getBoundingClientRect").mockReturnValue(
      new DOMRect(0, 340, 600, 80),
    );
    expect(captureViewportAnchor(root)).toEqual({
      kind: "verse",
      localPage: 1,
      verseKey: "2:1",
      viewportPoint: 340,
      ratio: 0,
    });
  });
});
