import { describe, expect, it, vi } from "vite-plus/test";
import { GET } from "../+server";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

function locs(xml: string): string[] {
  return [...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map((match) => match[1]!);
}

describe("localized sitemap", () => {
  it("emits only published marketing locale-page pairs", async () => {
    const xml = await GET().text();
    const urls = locs(xml);

    expect(urls).toContain("https://easyquran.fyi/");
    expect(urls).toContain("https://easyquran.fyi/ar/");
    expect(urls).toContain("https://easyquran.fyi/about");
    expect(urls).not.toContain("https://easyquran.fyi/ar/about");
    expect(xml).toContain(
      '<xhtml:link rel="alternate" hreflang="ar" href="https://easyquran.fyi/ar/"/>',
    );
    expect(xml).toContain(
      '<xhtml:link rel="alternate" hreflang="x-default" href="https://easyquran.fyi/"/>',
    );
  });

  it("uses only English-UI reader locs, including bounded entry routes", async () => {
    const xml = await GET().text();
    const readerLocs = locs(xml).filter((href) => href.includes("/app"));

    expect(readerLocs).toHaveLength(1_298);
    expect(readerLocs).toContain("https://easyquran.fyi/en/app");
    expect(readerLocs).toContain("https://easyquran.fyi/en/app/juz");
    expect(readerLocs.every((href) => href.startsWith("https://easyquran.fyi/en/app"))).toBe(true);
    expect(readerLocs.some((href) => href.includes("/ar/app"))).toBe(false);
    expect(readerLocs.some((href) => href === "https://easyquran.fyi/app")).toBe(false);
  });

  it("keeps Quran-content translation alternates under the English UI canonical", async () => {
    const xml = await GET().text();

    expect(xml).toMatch(
      /hreflang="[^"]+" href="https:\/\/easyquran\.fyi\/en\/app\/[^"]+\/t\/[^"]+"/,
    );
    expect(xml).toMatch(
      /hreflang="[^"]+" href="https:\/\/easyquran\.fyi\/en\/app\/t\/[^"]+\/(?:page|juz)\/[^"]+"/,
    );
    expect(xml).not.toMatch(/href="https:\/\/easyquran\.fyi\/ar\/app\/[^"]*\/t\//);
    expect(xml).not.toMatch(/https:\/\/easyquran\.fyi\/en\/app[^<"]+\.(?:md|txt)/);
  });
});
