import { readFileSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

describe("marketing localization boundaries", () => {
  it("uses generated Paraglide messages only inside resolver functions", () => {
    const copySource = source("../marketing-copy.ts");
    const firstResolver = copySource.indexOf("export function marketingFooterLinks");

    expect(copySource).toContain('from "$lib/paraglide/messages.js"');
    expect(copySource).not.toMatch(/messages\/(?:en|ar)\.json/);
    expect(copySource.slice(0, firstResolver)).not.toMatch(/\bm\.[a-z0-9_]+\s*\(/i);
    expect(copySource).toContain("{ locale }");
  });

  it("keeps shared chrome independent from Paraglide runtime", () => {
    const shared = [
      source("../../components/nav/Nav.svelte"),
      source("../../components/footer/Footer.svelte"),
      source("../../components/tweaks/Tweaks.svelte"),
      source("../../components/brand/Brand.svelte"),
    ].join("\n");

    expect(shared).not.toContain("$lib/paraglide");
    expect(shared).not.toMatch(/messages\/(?:en|ar)\.json/);
  });

  it("keeps Quran quotation verbatim and explicitly Arabic RTL", () => {
    const footer = source("../../components/footer/Footer.svelte");

    expect(footer).toContain('lang="ar" dir="rtl"');
    expect(footer).toContain("وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ");
  });

  it("routes landing reader links through the reader home helper", () => {
    const landing = source("../../../routes/(marketing)/+page.svelte");

    expect(landing).toContain("marketingReaderHomeHref(locale)");
    expect(landing).not.toMatch(/href=["']\/(?:en|ar)?\/?app/);
  });
});
