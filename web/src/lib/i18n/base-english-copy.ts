import { MARKETING_PAGES, type MarketingPageId } from "$lib/config/site-structure";

export type BaseEnglishPageId = MarketingPageId | "app";

export interface BaseEnglishPageCopy {
  readonly label: string;
  readonly title: string;
  readonly description: string;
}

/** English-only adapter for legacy pages and text representations outside the published i18n matrix. */
export function baseEnglishPageCopy(pageId: BaseEnglishPageId): BaseEnglishPageCopy {
  switch (pageId) {
    case "home":
      return {
        label: "Home",
        title: "EasyQuran · the Quran, made easy to read",
        description: "Read the Quran. Free, no ads, and fast — nothing in the way of the text.",
      };
    case "about":
      return {
        label: "About",
        title: "About · EasyQuran",
        description: "What EasyQuran is, who it's for, and how it's built.",
      };
    case "faq":
      return {
        label: "FAQ",
        title: "FAQ · EasyQuran",
        description: "Questions about EasyQuran, answered plainly.",
      };
    case "contact":
      return {
        label: "Contact",
        title: "Contact · EasyQuran",
        description: "Get in touch — bug reports, script corrections and feature ideas.",
      };
    case "privacy":
      return {
        label: "Privacy",
        title: "Privacy · EasyQuran",
        description: "What EasyQuran collects, what it doesn't, and why.",
      };
    case "terms":
      return {
        label: "Terms",
        title: "Terms · EasyQuran",
        description: "The terms under which EasyQuran is provided.",
      };
    case "app":
      return { label: "Read", title: "EasyQuran", description: "Read the Quran." };
  }
}

export function baseEnglishPageCopyForPath(path: string): BaseEnglishPageCopy | null {
  const page = MARKETING_PAGES.find((candidate) => candidate.href === path);
  if (page) return baseEnglishPageCopy(page.id);
  return path === "/app" ? baseEnglishPageCopy("app") : null;
}
