import { commandPalette } from "#lib/stores/command-palette.svelte.js";
import { searchRecents } from "#lib/stores/search-recents.svelte.js";

import { PaletteGroups } from "../groups";
import type { PaletteEntry, PaletteSource } from "../types";

const SOURCE_ID = "search.recents";
/** Recents sit above text hits but below jump-to references (groups.ts order). */
const RECENTS_LIMIT = 5;

/**
 * Recent local queries, shown only on an empty palette so they never crowd
 * live matches. Labels stay English — see the palette-label precedent in
 * site-routes.ts. Selecting one fills the palette input; it never navigates.
 */
export const searchRecentsSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.RecentQueries],
  limit: RECENTS_LIMIT,

  enabled({ parsed }) {
    return parsed.isEmpty;
  },

  entries(): PaletteEntry[] {
    return searchRecents.queries.map((q) => ({
      id: `${SOURCE_ID}:${q}`,
      sourceId: SOURCE_ID,
      groupId: PaletteGroups.RecentQueries.id,
      label: q,
      icon: "search",
      score: 0,
      run: () => commandPalette.setQuery(q),
    }));
  },
};
