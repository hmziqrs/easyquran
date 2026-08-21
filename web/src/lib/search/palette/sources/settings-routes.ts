import { PaletteGroups } from "../groups";
import { byScore, scoreFields } from "../scoring";
import type { PaletteEntry, PaletteSource } from "../types";

const SOURCE_ID = "settings.routes";

const SETTINGS_PATH = "/app/settings";

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

    const entries: PaletteEntry[] = [
      {
        id: `${SOURCE_ID}:settings`,
        sourceId: SOURCE_ID,
        groupId: PaletteGroups.Settings.id,
        label,
        detail,
        icon: "rows",
        score,
        href: SETTINGS_PATH,
        dedupeKey: `href:${SETTINGS_PATH}`,
      },
    ];

    return byScore(entries);
  },
};
