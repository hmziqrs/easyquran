import type { Pathname } from "$app/types";
import { authState } from "$lib/auth/auth-state.svelte";
import { MARKETING_PAGES } from "$lib/config/site-structure";
import { baseEnglishPageCopy } from "$lib/i18n/base-english-copy";
import { PaletteGroups } from "../groups";
import { byScore, scoreFields } from "../scoring";
import type { PaletteEntry, PaletteSource } from "../types";

const SOURCE_ID = "site.routes";

export interface SiteRoute {
  href: Pathname;
  label: string;
  detail?: string;
  keywords?: readonly string[];
}

/**
 * `MARKETING_PAGES` is the source of truth for site nav but types hrefs as
 * plain strings; every entry is a real route, so this is the one place that
 * assumption is stated.
 */
const STATIC_ROUTES: readonly SiteRoute[] = [
  {
    href: "/app",
    label: "Read the Quran",
    detail: "Reader",
    keywords: ["reader", "read", "mushaf", "quran", "open app"],
  },
  ...MARKETING_PAGES.map((page) => ({
    href: page.href as Pathname,
    label: baseEnglishPageCopy(page.id).label,
    detail: "Page",
    keywords: [page.id],
  })),
];

const accountRoute = (): SiteRoute =>
  authState.authenticated
    ? { href: "/account", label: "Account", detail: "Page", keywords: ["profile", "settings"] }
    : {
        href: "/login",
        label: "Sign in",
        detail: "Page",
        keywords: ["login", "sign up", "account"],
      };

/** Site navigation. Reads auth state so the row is "Account" or "Sign in". */
export const siteRoutesSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.Site],
  limit: 6,

  entries({ parsed }) {
    const routes = [...STATIC_ROUTES, accountRoute()];
    const entries: PaletteEntry[] = [];

    for (const route of routes) {
      const score = parsed.isEmpty
        ? 0
        : scoreFields([route.label, route.href, ...(route.keywords ?? [])], parsed.text);
      if (!parsed.isEmpty && score === 0) continue;
      entries.push({
        id: `${SOURCE_ID}:${route.href}`,
        sourceId: SOURCE_ID,
        groupId: PaletteGroups.Site.id,
        label: route.label,
        detail: route.detail,
        icon: "note",
        score,
        href: route.href,
        dedupeKey: `href:${route.href}`,
      });
    }

    return byScore(entries);
  },
};
