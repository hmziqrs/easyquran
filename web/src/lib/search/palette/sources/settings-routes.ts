import type { Pathname } from "$app/types";
import type { UiLocale } from "$lib/i18n/locales";
import { publicHref } from "$lib/i18n/public-href";
import { readerHrefFor } from "$lib/i18n/reader";
import { getLocale } from "$lib/paraglide/runtime.js";

import { PaletteGroups } from "../groups";
import { byScore, scoreFields } from "../scoring";
import type { PaletteEntry, PaletteSource } from "../types";

const SOURCE_ID = "settings.routes";

function settingsHref(): Pathname {
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;
  // SAFETY: publicHref returns a validated route string and readerHrefFor only yields internal app pathnames, so the result is a valid Pathname.
  return publicHref(readerHrefFor(locale, "/app/settings")) as Pathname;
}

/** Settings navigation. Labels stay English — see the palette-label precedent in site-routes.ts. */
export const settingsRoutesSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.Settings],
  limit: 2,

  entries({ parsed }) {
    const label = "Settings";
    const detail = "Storage, appearance, reading, privacy";
    const score = parsed.isEmpty
      ? 0
      : scoreFields(
          [label, "storage", "appearance", "fonts", "theme", "privacy", "account"],
          parsed.text,
        );
    if (!parsed.isEmpty && score === 0) return [];

    const target = settingsHref();
    const entries: PaletteEntry[] = [
      {
        id: `${SOURCE_ID}:settings`,
        sourceId: SOURCE_ID,
        groupId: PaletteGroups.Settings.id,
        label,
        detail,
        icon: "rows",
        score,
        href: target,
        dedupeKey: `href:${target}`,
      },
    ];

    return byScore(entries);
  },
};
