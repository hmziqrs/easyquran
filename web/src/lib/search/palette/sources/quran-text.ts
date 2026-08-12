import { quranSearch } from "$lib/quran/search";
import { MIN_QUERY_LEN } from "$lib/quran/search/normalize";
import {
  SearchHitKind,
  SearchProvider,
  searchHitAnchorAyah,
  searchHitKey,
  searchHitSurah,
  searchHitText,
} from "$lib/quran/search/types";
import { QURAN_ALIASES } from "../aliases";
import { PaletteGroups } from "../groups";
import { residualText } from "../query";
import { ayahHref, openVerse } from "../quran-nav";
import type { PaletteEntry, PaletteSource } from "../types";

const SOURCE_ID = "quran.text";
const LIMIT = 8;

/**
 * Full-text Quran search. Async because it goes to the OPFS worker or the API
 * (see `$lib/quran/search`), so the engine debounces and cancels it; the
 * catalogue-only sources keep answering on every keystroke meanwhile.
 */
export const quranTextSource: PaletteSource = {
  id: SOURCE_ID,
  groups: [PaletteGroups.QuranText],
  limit: LIMIT,

  /**
   * Needs real free text, not a coordinate: `2:255`, `juz 5` and `112` are
   * already answered exactly by the catalogue sources, and sending them to the
   * worker only burns a round-trip per keystroke to match nothing.
   */
  enabled: ({ parsed }) => residualText(parsed, QURAN_ALIASES).length >= MIN_QUERY_LEN,

  async search(query, signal) {
    const response = await quranSearch(query.parsed.text, { limit: query.limit });
    if (signal.aborted) return [];

    // The `Names` provider is the degraded surah-name fallback used when no
    // Quran text is available locally; `quran.surahs` already covers that
    // ground from the catalogue, so echoing it here would only duplicate rows.
    if (response.source === SearchProvider.Names) return [];

    const entries: PaletteEntry[] = [];
    for (const hit of response.results) {
      const num = searchHitSurah(hit);
      const ayah = searchHitAnchorAyah(hit);
      const surah = query.quranData.surahByNum(num);
      if (!surah) continue;
      const href = ayahHref(query.routeContext, query.quranData, num, ayah);
      if (!href) continue;
      const text = searchHitText(hit);
      const opener = hit.kind === SearchHitKind.Opener;
      entries.push({
        id: `${SOURCE_ID}:${searchHitKey(hit)}`,
        sourceId: SOURCE_ID,
        groupId: PaletteGroups.QuranText.id,
        label: opener ? `${surah.name} · Surah opener` : `${surah.name} ${num}:${ayah}`,
        detail: opener ? "Opener" : undefined,
        preview: text ? { text, highlights: hit.highlights } : undefined,
        icon: "search",
        score: 0.7,
        href,
        run: openVerse(num, ayah),
        dedupeKey: opener ? `opener:${num}` : `ayah:${num}:${ayah}`,
      });
    }
    return entries;
  },
};
