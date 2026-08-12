import { readFileSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";
import { resolveAboutCopy } from "$lib/i18n/about-copy";
import { resolveContactCopy } from "$lib/i18n/contact-copy";
import { resolveFaqCopy } from "$lib/i18n/faq-copy";
import { resolvePrivacyCopy, resolvePrivacySummary } from "$lib/i18n/privacy-copy";
import { resolveTermsCopy } from "$lib/i18n/terms-copy";

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), "utf8");
}

describe("marketing page copy", () => {
  it("translates every page and keeps structural IDs locale-independent", () => {
    const about = { en: resolveAboutCopy("en"), ar: resolveAboutCopy("ar") };
    const faq = { en: resolveFaqCopy("en"), ar: resolveFaqCopy("ar") };
    const contact = { en: resolveContactCopy("en"), ar: resolveContactCopy("ar") };
    const privacy = { en: resolvePrivacyCopy("en"), ar: resolvePrivacyCopy("ar") };
    const terms = { en: resolveTermsCopy("en"), ar: resolveTermsCopy("ar") };

    expect(about.ar.heading).not.toBe(about.en.heading);
    expect(faq.ar.entries[0]?.q).not.toBe(faq.en.entries[0]?.q);
    expect(contact.ar.intro).not.toBe(contact.en.intro);
    expect(privacy.ar.sections[0]?.body).not.toBe(privacy.en.sections[0]?.body);
    expect(terms.ar.sections[0]?.body).not.toBe(terms.en.sections[0]?.body);
    expect(resolvePrivacySummary("ar")).not.toBe(resolvePrivacySummary("en"));

    expect(about.ar.stats.map((stat) => stat.id)).toEqual(about.en.stats.map((stat) => stat.id));
    expect(faq.ar.entries.map((entry) => entry.id)).toEqual(
      faq.en.entries.map((entry) => entry.id),
    );
    expect(privacy.ar.sections.map((s) => s.id)).toEqual(privacy.en.sections.map((s) => s.id));
    expect(terms.ar.sections.map((s) => s.id)).toEqual(terms.en.sections.map((s) => s.id));
  });

  it("renders the legal date through the parameterized message", () => {
    expect(resolvePrivacyCopy("en").updated).toBe("Last updated 30 July 2026");
    expect(resolveTermsCopy("ar").updated).toContain("آخر تحديث");
  });

  it("gives every page its own namespace so pages never pay for each other", () => {
    const modules = {
      "about-copy.ts": ["about"],
      "faq-copy.ts": ["faq"],
      "contact-copy.ts": ["contact"],
      "privacy-copy.ts": ["privacy"],
      "terms-copy.ts": ["terms"],
      "legal-copy.ts": ["legal"],
    } as const;

    for (const [file, allowed] of Object.entries(modules)) {
      const used = [...source(`../${file}`).matchAll(/\$lib\/i18n\/m\/([\w-]+)/g)].map((m) => m[1]);
      expect(new Set(used), `${file} may only import ${allowed.join(", ")}`).toEqual(
        new Set(allowed),
      );
    }
  });

  it("keeps localized pages off the deleted English-only content module", () => {
    for (const page of ["about", "faq", "contact", "privacy", "terms"]) {
      const template = source(`../../../routes/(marketing)/${page}/+page.svelte`);
      expect(template).not.toContain("$lib/data/content");
      expect(template).toContain("marketingLocaleFromPath");
      expect(template).toContain("inLanguage={locale}");
    }
  });
});
