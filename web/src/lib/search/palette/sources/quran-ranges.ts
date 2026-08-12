import { PaletteGroups } from "../groups";
import { hasKeyword } from "../query";
import { JUZ_COUNT, MUSHAF_PAGE_COUNT, juzHref, pageHref } from "../quran-nav";
import type { PaletteEntry, PaletteSource } from "../types";
import { JUZ_ALIASES, PAGE_ALIASES } from "./quran-reference";

const SOURCE_ID = "quran.ranges";

/**
 * Browsable juz and page lists for a bare `juz` / `page` keyword with no number
 * yet — the palette should be explorable, not only answerable. Once a number is
 * typed, `quran.reference` takes over with the exact hit.
 */
export const quranRangesSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.Ranges],
  limit: 6,

  enabled: ({ parsed }) =>
    parsed.numbers.length === 0 &&
    (hasKeyword(parsed, JUZ_ALIASES) || hasKeyword(parsed, PAGE_ALIASES)),

  entries(query) {
    const { parsed, limit } = query;
    const wantsJuz = hasKeyword(parsed, JUZ_ALIASES);
    const count = Math.max(
      1,
      wantsJuz ? Math.min(limit, JUZ_COUNT) : Math.min(limit, MUSHAF_PAGE_COUNT),
    );
    const entries: PaletteEntry[] = [];

    for (let n = 1; n <= count; n += 1) {
      entries.push(
        wantsJuz
          ? {
              id: `${SOURCE_ID}:juz:${n}`,
              sourceId: SOURCE_ID,
              groupId: PaletteGroups.Ranges.id,
              label: `Juz ${n}`,
              detail: "Juz",
              icon: "rows",
              score: 0.5,
              href: juzHref(query.routeContext, n),
              dedupeKey: `juz:${n}`,
            }
          : {
              id: `${SOURCE_ID}:page:${n}`,
              sourceId: SOURCE_ID,
              groupId: PaletteGroups.Ranges.id,
              label: `Page ${n}`,
              detail: "Mushaf page",
              icon: "rows",
              score: 0.5,
              href: pageHref(query.routeContext, n),
              dedupeKey: `page:${n}`,
            },
      );
    }

    return entries;
  },
};
