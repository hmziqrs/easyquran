import { afterEach, describe, expect, it, vi } from "vite-plus/test";

// Mutable $app/state stand-in: tests point the share at arabic or /t/** routes.
const pageState = vi.hoisted(() => ({
  url: new URL("https://eq.test/surah-2"),
  params: {},
}));
vi.mock("$app/state", () => ({ page: pageState }));
vi.mock("$app/env", () => ({ browser: true }));

import { createReaderShare } from "../reader-share.svelte";

type WriteText = (text: string) => Promise<void>;

function stubNavigator(
  share?: (data: { title?: string; text?: string; url?: string }) => Promise<void>,
) {
  const writeText = vi.fn<WriteText>().mockResolvedValue(undefined);
  const nav = { clipboard: { writeText } };
  if (share) Object.assign(nav, { share });
  vi.stubGlobal("navigator", nav);
  return writeText;
}

afterEach(() => {
  vi.unstubAllGlobals();
  pageState.params = {};
});

describe("reader-share deep links (FLOW-05)", () => {
  it("copyVerse appends the absolute ayah deep link", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator();
    await share.copyVerse("2:255", "ٱللَّهُ لَا إِلَٰهَ");
    const written = writeText.mock.calls[0]?.[0] ?? "";
    expect(written).toContain("https://eq.test/surah-2#ayah-2-255");
    expect(written).toContain("Surah 2 2:255");
  });

  it("shareVerse passes url to navigator.share and keeps the link in the text", async () => {
    const share = createReaderShare();
    const shareSpy =
      vi.fn<(data: { title?: string; text?: string; url?: string }) => Promise<void>>();
    stubNavigator(shareSpy);
    const result = await share.shareVerse("2:255", "ٱللَّهُ لَا إِلَٰهَ");
    expect(result).toBe("shared");
    expect(shareSpy.mock.calls[0]?.[0]?.url).toBe("https://eq.test/surah-2#ayah-2-255");
    expect(shareSpy.mock.calls[0]?.[0]?.text).toContain("https://eq.test/surah-2#ayah-2-255");
  });

  it("no share API falls back to clipboard with the full body", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator();
    const result = await share.shareVerse("2:255", "text");
    expect(result).toBe("copied");
    expect(writeText.mock.calls[0]?.[0]).toContain("https://eq.test/surah-2#ayah-2-255");
  });

  it("dismissed share sheet is silent (no message, nothing copied)", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator(() =>
      Promise.reject(new DOMException("user cancelled", "AbortError")),
    );
    const result = await share.shareVerse("2:255", "text");
    expect(result).toBe("cancelled");
    expect(writeText).not.toHaveBeenCalled();
  });

  it("failed share copies the link and says so", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator(() =>
      Promise.reject(new DOMException("denied", "NotAllowedError")),
    );
    const result = await share.shareVerse("2:255", "text");
    expect(result).toBe("link-copied");
    expect(writeText.mock.calls[0]?.[0]).toBe("https://eq.test/surah-2#ayah-2-255");
  });

  it("failed share with dead clipboard reports failed", async () => {
    const share = createReaderShare();
    vi.stubGlobal("navigator", {
      clipboard: { writeText: () => Promise.reject(new DOMException("denied", "NotAllowedError")) },
      share: () => Promise.reject(new DOMException("denied", "NotAllowedError")),
    });
    const result = await share.shareVerse("2:255", "text");
    expect(result).toBe("failed");
  });

  it("deep link preserves the open translation context", async () => {
    pageState.params = { lang: "en", translator: "sahih" };
    const share = createReaderShare();
    const writeText = stubNavigator();
    await share.copyVerse("2:255", "text");
    expect(writeText.mock.calls[0]?.[0]).toContain("https://eq.test/surah-2/t/en/sahih#ayah-2-255");
  });
});

describe("reader-share range variant (Feature 7.5)", () => {
  it("joins verse texts + refs with the canonical ?ayah= permalink", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator();
    const result = await share.shareRange("2:285", "2:286", [
      { key: "2:285", text: "آمَنَ الرَّسُولُ" },
      { key: "2:286", text: "لَا يُكَلِّفُ ٱللَّهُ نَفْسًا" },
    ]);
    expect(result).toBe("copied");
    const written = writeText.mock.calls[0]?.[0] ?? "";
    expect(written).toContain("آمَنَ الرَّسُولُ\nSurah 2 2:285");
    expect(written).toContain("لَا يُكَلِّفُ ٱللَّهُ نَفْسًا\nSurah 2 2:286");
    expect(written.endsWith("https://eq.test/surah-2?ayah=2:285-2:286")).toBe(true);
  });

  it("shareRange hands navigator.share the canonical url", async () => {
    const share = createReaderShare();
    const shareSpy =
      vi.fn<(data: { title?: string; text?: string; url?: string }) => Promise<void>>();
    stubNavigator(shareSpy);
    const result = await share.shareRange("2:285", "2:286", [
      { key: "2:285", text: "a" },
      { key: "2:286", text: "b" },
    ]);
    expect(result).toBe("shared");
    expect(shareSpy.mock.calls[0]?.[0]?.url).toBe("https://eq.test/surah-2?ayah=2:285-2:286");
    expect(shareSpy.mock.calls[0]?.[0]?.title).toBe("Surah 2 2:285–286");
  });

  it("failed range share still copies the canonical link", async () => {
    const share = createReaderShare();
    const writeText = stubNavigator(() =>
      Promise.reject(new DOMException("denied", "NotAllowedError")),
    );
    const result = await share.shareRange("2:285", "2:286", [{ key: "2:285", text: "a" }]);
    expect(result).toBe("link-copied");
    expect(writeText.mock.calls[0]?.[0]).toBe("https://eq.test/surah-2?ayah=2:285-2:286");
  });
});
