import { prefs } from "$lib/stores/prefs.svelte";
import { reader } from "$lib/stores/reader.svelte";
import { PaletteGroups } from "../groups";
import { ayahHref, openVerse } from "../quran-nav";
import { byScore, scoreFields } from "../scoring";
import type { PaletteEntry, PaletteQuery, PaletteSource } from "../types";

const SOURCE_ID = "app.actions";

interface ActionDef {
  key: string;
  label: string;
  detail?: string;
  icon: PaletteEntry["icon"];
  keywords: readonly string[];
  run: () => void;
}

const ACTIONS: readonly ActionDef[] = [
  {
    key: "toggle-theme",
    label: "Toggle theme",
    detail: "Switch between dark and light",
    icon: "moon",
    keywords: ["dark mode", "light mode", "appearance", "theme"],
    run: () => prefs.toggleTheme(),
  },
];

const CONTINUE_KEYWORDS = ["continue reading", "resume", "last read", "where i left off"] as const;

/**
 * Resume where the reader left off. Lives in "Jump to" rather than "Actions"
 * because it is a coordinate, and it is offered unscored on an empty query so
 * opening the palette always has somewhere useful to go.
 */
function continueReading(query: PaletteQuery): PaletteEntry | null {
  const lastRead = reader.lastRead;
  if (!lastRead) return null;
  const surah = query.quranData.surahByNum(lastRead.num);
  if (!surah) return null;
  const href = ayahHref(query.routeContext, query.quranData, lastRead.num, lastRead.n);
  if (!href) return null;

  const score = query.parsed.isEmpty ? 0 : scoreFields(CONTINUE_KEYWORDS, query.parsed.text);
  if (!query.parsed.isEmpty && score === 0) return null;

  return {
    id: `${SOURCE_ID}:continue-reading`,
    sourceId: SOURCE_ID,
    groupId: PaletteGroups.JumpTo.id,
    label: "Continue reading",
    detail: `${surah.name} ${lastRead.num}:${lastRead.n}`,
    icon: "continuous",
    score,
    href,
    run: openVerse(lastRead.num, lastRead.n),
    dedupeKey: `ayah:${lastRead.num}:${lastRead.n}`,
  };
}

/** App-level commands and the resume-reading shortcut. */
export const appActionsSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.Actions, PaletteGroups.JumpTo],
  limit: 5,

  entries(query) {
    const { parsed } = query;
    const entries: PaletteEntry[] = [];

    const resume = continueReading(query);
    if (resume) entries.push(resume);

    for (const action of ACTIONS) {
      const score = parsed.isEmpty
        ? 0
        : scoreFields([action.label, ...action.keywords], parsed.text);
      if (!parsed.isEmpty && score === 0) continue;
      entries.push({
        id: `${SOURCE_ID}:${action.key}`,
        sourceId: SOURCE_ID,
        groupId: PaletteGroups.Actions.id,
        label: action.label,
        detail: action.detail,
        icon: action.icon,
        score,
        run: action.run,
      });
    }

    return byScore(entries);
  },
};
