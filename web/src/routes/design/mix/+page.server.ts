import { uniq } from "es-toolkit";

import { loadMixData } from "../_variants/mix/data.server";
import type { PageServerLoad } from "./$types";

// Reads `?t=` (the translation set), so it renders on request, never at build time.
export const prerender = false;

const DEFAULT_TRANSLATIONS = ["en.sahih", "ur.jalandhry"];
// Same cap as the live reader's stacked translations (STACKED_MAX_EXTRAS).
const MAX_TRANSLATIONS = 5;

export const load: PageServerLoad = ({ url }) => {
  // No `t` → the default pair; an empty `t=` → Arabic only (the popup's "Clear all").
  const raw = url.searchParams.get("t");
  const ids =
    raw === null
      ? DEFAULT_TRANSLATIONS
      : uniq(raw.split(",").map((id) => id.trim()).filter(Boolean)).slice(0, MAX_TRANSLATIONS);
  return { ...loadMixData(ids), maxTranslations: MAX_TRANSLATIONS };
};
